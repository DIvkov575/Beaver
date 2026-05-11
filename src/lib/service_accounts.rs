//! Service account provisioning and resource-scoped IAM bindings.
//!
//! Beaver creates one runtime SA per long-running component (Vector on
//! Cloud Run, Dataflow workers) and grants each only the IAM bindings it
//! actually needs, scoped to the specific Pub/Sub subscription/topic, GCS
//! bucket, etc. No project-wide Editor; the runtime SAs cannot reach
//! anything outside their declared interface even if compromised.

use std::process::Command;

use anyhow::{anyhow, Result};
use log::info;

use crate::lib::config::Config;

pub struct CreatedSa {
    pub account_id: String,
    pub email: String,
}

/// Idempotent: if the SA already exists, returns its email without re-creating.
pub fn create_sa(account_id: &str, display_name: &str, config: &Config) -> Result<CreatedSa> {
    let email = format!("{}@{}.iam.gserviceaccount.com", account_id, config.project);

    let exists = Command::new("gcloud")
        .args([
            "iam", "service-accounts", "describe", &email,
            "--project", &config.project,
        ])
        .output()?;

    if !exists.status.success() {
        info!("creating service account {}", email);
        let out = Command::new("gcloud")
            .args([
                "iam", "service-accounts", "create", account_id,
                "--display-name", display_name,
                "--project", &config.project,
            ])
            .output()?;
        if !out.status.success() {
            return Err(anyhow!(
                "service account create failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        // IAM has eventual consistency — describe-after-create can return
        // NOT_FOUND for a few seconds. Poll until visible.
        for _ in 0..10 {
            let check = Command::new("gcloud")
                .args(["iam", "service-accounts", "describe", &email,
                       "--project", &config.project])
                .output()?;
            if check.status.success() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    Ok(CreatedSa { account_id: account_id.to_string(), email })
}

pub fn delete_sa(email: &str, project: &str) -> Result<()> {
    info!("deleting service account {}", email);
    let out = Command::new("gcloud")
        .args([
            "iam", "service-accounts", "delete", email,
            "--quiet", "--project", project,
        ])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Soft-deleted SAs return PERMISSION_DENIED with "or it may not exist"
        // wrapping; treat that the same as NOT_FOUND for idempotency.
        if stderr.contains("NOT_FOUND")
            || stderr.contains("does not exist")
            || stderr.contains("may not exist")
        {
            return Ok(());
        }
        return Err(anyhow!("service account delete failed: {}", stderr));
    }
    Ok(())
}

/// Retries gcloud IAM binding commands on "service account does not exist"
/// errors — IAM has visible-via-describe-but-not-via-policy lag of ~10-20s
/// after SA creation, and any binding hitting that window fails spuriously.
fn run_iam_grant_with_retry(args: &[&str], kind: &str) -> Result<()> {
    for attempt in 0..6 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        let out = Command::new("gcloud").args(args).output()?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !stderr.contains("does not exist") && !stderr.contains("NOT_FOUND") {
            return Err(anyhow!("{} failed: {}", kind, stderr));
        }
        info!("{} retry {}: SA not yet visible to IAM", kind, attempt + 1);
    }
    Err(anyhow!("{} failed after retries: SA propagation timed out", kind))
}

pub fn grant_pubsub_topic(topic: &str, sa_email: &str, role: &str, project: &str) -> Result<()> {
    let member = format!("serviceAccount:{}", sa_email);
    let args = [
        "pubsub", "topics", "add-iam-policy-binding", topic,
        "--member", &member, "--role", role,
        "--project", project,
    ];
    run_iam_grant_with_retry(&args, &format!("topic IAM grant ({} on {})", role, topic))
}

pub fn grant_pubsub_subscription(sub: &str, sa_email: &str, role: &str, project: &str) -> Result<()> {
    let member = format!("serviceAccount:{}", sa_email);
    let args = [
        "pubsub", "subscriptions", "add-iam-policy-binding", sub,
        "--member", &member, "--role", role,
        "--project", project,
    ];
    run_iam_grant_with_retry(&args, &format!("subscription IAM grant ({} on {})", role, sub))
}

pub fn grant_bucket(bucket: &str, sa_email: &str, role: &str) -> Result<()> {
    let member = format!("serviceAccount:{}", sa_email);
    let target = format!("gs://{}", bucket);
    let args = [
        "storage", "buckets", "add-iam-policy-binding", &target,
        "--member", &member, "--role", role,
    ];
    run_iam_grant_with_retry(&args, &format!("bucket IAM grant ({} on {})", role, bucket))
}

pub fn grant_project(project: &str, sa_email: &str, role: &str) -> Result<()> {
    let member = format!("serviceAccount:{}", sa_email);
    let args = [
        "projects", "add-iam-policy-binding", project,
        "--member", &member, "--role", role,
        "--condition=None",
    ];
    run_iam_grant_with_retry(&args, &format!("project IAM grant ({})", role))
}

pub fn project_number(project: &str) -> Result<String> {
    let out = Command::new("gcloud")
        .args(["projects", "describe", project, "--format=value(projectNumber)"])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "project number lookup failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Grants `roles/bigquery.dataEditor` to the Pub/Sub Service Agent so that
/// BigQuery subscriptions can actually write to BQ tables. In a permissive
/// project this is a no-op (the binding may already exist); in a hardened
/// project it's required for messages to flow at all.
pub fn grant_pubsub_to_bq(project: &str) -> Result<()> {
    let pn = project_number(project)?;
    let agent = format!("service-{}@gcp-sa-pubsub.iam.gserviceaccount.com", pn);
    grant_project(project, &agent, "roles/bigquery.dataEditor")
}


#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::lib::test_helpers::{service_account_exists, test_config};
    use crate::lib::utilities::random_tag;

    #[test]
    #[ignore]
    fn create_then_delete_leaves_no_sa() {
        let config = test_config();
        let id = format!("beaver-it-{}", random_tag(6));
        let sa = create_sa(&id, "Beaver IT", &config).expect("create");
        assert!(
            service_account_exists(&sa.email, &config.project),
            "{} should exist after create", sa.email
        );

        // Idempotency: second create returns same email, no error.
        let again = create_sa(&id, "Beaver IT", &config).expect("idempotent create");
        assert_eq!(again.email, sa.email);

        delete_sa(&sa.email, &config.project).expect("delete");
        assert!(
            !service_account_exists(&sa.email, &config.project),
            "{} should be gone", sa.email
        );

        // Delete idempotency: deleting an already-deleted SA must not error.
        delete_sa(&sa.email, &config.project).expect("idempotent delete");
    }
}

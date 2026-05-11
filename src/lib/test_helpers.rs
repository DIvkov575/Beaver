//! Helpers for `#[ignore]` integration tests. Run with `cargo test -- --ignored`.
//! Expect `gcloud` to be authenticated; override the test project/region via
//! `BEAVER_TEST_PROJECT` and `BEAVER_TEST_REGION`.

#![cfg(test)]
#![allow(dead_code)]

use crate::lib::config::Config;
use crate::lib::resources::Resources;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub fn test_project() -> String {
    std::env::var("BEAVER_TEST_PROJECT").unwrap_or_else(|_| "neon-circle-400322".to_string())
}

pub fn test_region() -> String {
    std::env::var("BEAVER_TEST_REGION").unwrap_or_else(|_| "us-east1".to_string())
}

pub fn test_config() -> Config {
    Config::new(&test_region(), &test_project(), None)
}

/// `Resources` rooted at a tempdir; the returned guard must outlive the test
/// so `Tracker::persist` keeps writing to a valid path.
pub fn tempdir_resources() -> (TempDir, Resources) {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("artifacts")).unwrap();
    let res = Resources::empty(&test_config(), dir.path());
    (dir, res)
}

fn gcloud_describe_succeeds(args: &[&str]) -> bool {
    let out = Command::new("gcloud")
        .args(args)
        .output()
        .expect("gcloud failed to spawn");
    out.status.success()
}

pub fn bq_dataset_exists(dataset_id: &str, project: &str) -> bool {
    let out = Command::new("bq")
        .args(["show", "--format=none", &format!("{}:{}", project, dataset_id)])
        .output()
        .expect("bq failed to spawn");
    out.status.success()
}

pub fn pubsub_topic_exists(id: &str, project: &str) -> bool {
    gcloud_describe_succeeds(&["pubsub", "topics", "describe", id, "--project", project])
}

pub fn pubsub_subscription_exists(id: &str, project: &str) -> bool {
    gcloud_describe_succeeds(&["pubsub", "subscriptions", "describe", id, "--project", project])
}

pub fn bucket_exists(name: &str) -> bool {
    let out = Command::new("gcloud")
        .args(["storage", "buckets", "describe", &format!("gs://{}", name)])
        .output()
        .expect("gcloud failed");
    out.status.success()
}

pub fn artifact_repo_exists(name: &str, project: &str, region: &str) -> bool {
    gcloud_describe_succeeds(&[
        "artifacts", "repositories", "describe", name,
        "--location", region, "--project", project,
    ])
}

pub fn artifact_image_exists(full_url: &str, project: &str) -> bool {
    let out = Command::new("gcloud")
        .args(["artifacts", "docker", "images", "describe", full_url,
               "--project", project])
        .output()
        .expect("gcloud failed");
    out.status.success()
}

pub fn notification_channel_exists(id: &str, project: &str) -> bool {
    gcloud_describe_succeeds(&[
        "beta", "monitoring", "channels", "describe", id, "--project", project,
    ])
}

pub fn alert_policy_exists(id: &str, project: &str) -> bool {
    gcloud_describe_succeeds(&[
        "alpha", "monitoring", "policies", "describe", id, "--project", project,
    ])
}

/// Returns true only when the SA is in the active list. GCP soft-deletes
/// SAs (kept for ~30 days), so `describe` succeeds long after `delete` —
/// `list` is the right "is it actually usable" check.
pub fn service_account_exists(email: &str, project: &str) -> bool {
    let out = Command::new("gcloud")
        .args([
            "iam", "service-accounts", "list",
            "--project", project,
            "--filter", &format!("email={}", email),
            "--format=value(email)",
        ])
        .output()
        .expect("gcloud failed");
    !String::from_utf8_lossy(&out.stdout).trim().is_empty()
}

pub fn crs_service_exists(name: &str, project: &str, region: &str) -> bool {
    gcloud_describe_succeeds(&[
        "run", "services", "describe", name,
        "--region", region, "--project", project,
    ])
}

pub fn scheduler_job_exists(name: &str, project: &str, region: &str) -> bool {
    gcloud_describe_succeeds(&[
        "scheduler", "jobs", "describe", name,
        "--location", region, "--project", project,
    ])
}


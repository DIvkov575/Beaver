//! Pre-deploy environment checks. Run at the top of every deploy so missing
//! prerequisites surface as named errors with copy-pasteable fixes instead of
//! cryptic gcloud failures mid-flight.

use std::process::Command;

use anyhow::{anyhow, Result};
use log::{info, warn};

use crate::lib::config::Config;

const REQUIRED_APIS: &[&str] = &[
    "cloudbuild.googleapis.com",
    "artifactregistry.googleapis.com",
    "run.googleapis.com",
    "pubsub.googleapis.com",
    "bigquery.googleapis.com",
    "storage.googleapis.com",
    "dataflow.googleapis.com",
    "monitoring.googleapis.com",
    "iam.googleapis.com",
];

pub fn run(config: &Config) -> Result<()> {
    probe_auth()?;
    set_active_project(&config.project)?;
    enable_apis(&config.project)?;
    if let Some(billing) = &config.billing_account {
        link_billing(&config.project, billing)?;
    }
    ensure_alpha_components();
    Ok(())
}

fn probe_auth() -> Result<()> {
    let cli = Command::new("gcloud").args(["auth", "print-access-token"]).output()?;
    let adc = Command::new("gcloud")
        .args(["auth", "application-default", "print-access-token"])
        .output()?;
    if !cli.status.success() || !adc.status.success() {
        return Err(anyhow!(
            "Beaver needs both gcloud CLI auth and Application Default Credentials. Run:\n\n\
             \tgcloud auth login\n\
             \tgcloud auth application-default login\n\n\
             then retry."
        ));
    }
    Ok(())
}

fn set_active_project(project: &str) -> Result<()> {
    let out = Command::new("gcloud")
        .args(["config", "set", "project", project])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "failed to set active project: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn enable_apis(project: &str) -> Result<()> {
    info!(
        "ensuring {} required APIs are enabled (idempotent, ~30s on first run)...",
        REQUIRED_APIS.len()
    );
    let mut args: Vec<String> = vec!["services".into(), "enable".into()];
    for api in REQUIRED_APIS {
        args.push((*api).to_string());
    }
    args.push(format!("--project={}", project));
    let out = Command::new("gcloud")
        .args(args.iter().map(|s| s.as_str()))
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "API enablement failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// Idempotent link of a billing account. Returns `Ok(())` if the account is
/// already linked. Requires `roles/billing.user` on the account.
fn link_billing(project: &str, account: &str) -> Result<()> {
    info!("ensuring billing account {} is linked to {}", account, project);
    let out = Command::new("gcloud")
        .args([
            "beta", "billing", "projects", "link", project,
            "--billing-account", account,
        ])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("already") {
            return Ok(());
        }
        return Err(anyhow!("billing link failed: {}", stderr));
    }
    Ok(())
}

/// gcloud alpha is required for the monitoring/policy commands used by the
/// notifications feature. Soft check + soft install; if both fail, warn and
/// proceed — failures only matter for users actually using notifications.
fn ensure_alpha_components() {
    if let Ok(out) = Command::new("gcloud").args(["alpha", "--help"]).output() {
        if out.status.success() {
            return;
        }
    }
    info!("installing gcloud alpha components");
    match Command::new("gcloud")
        .args(["components", "install", "alpha", "--quiet"])
        .output()
    {
        Ok(out) if out.status.success() => {}
        _ => warn!(
            "could not install gcloud alpha components; \
             notifications will fail until you run `gcloud components install alpha`"
        ),
    }
}

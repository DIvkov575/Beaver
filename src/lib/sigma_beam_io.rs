//! Provision the Pub/Sub topics, BQ alerts table, BQ-push subscription,
//! and GCS rules prefix that sigma_beam's correlation_pipeline needs.

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use log::{info, warn};

use crate::lib::config::Config;
use crate::lib::pubsub::{create_named_pubsub_topic, delete_subscription, delete_topic};
use crate::lib::resources::Tracker;

const ALERTS_TOPIC: &str = "beaver-alerts";
const DLQ_TOPIC: &str = "beaver-dlq";
const ALERTS_TABLE: &str = "alerts";
const ALERTS_SUBSCRIPTION_PREFIX: &str = "beaver-alerts-to-bq";

/// Create alerts + DLQ topics, the alerts BQ table, and a push subscription
/// from the alerts topic into the alerts table. Idempotent at the topic
/// level (gcloud rejects "already exists" non-fatally — we treat it as ok).
pub fn create(tracker: &mut Tracker, config: &Config) -> Result<()> {
    info!("provisioning sigma_beam alerts + DLQ resources...");
    let project = config.project.clone();
    let dataset_id = tracker.resources().biq_query.dataset_id.clone();
    if dataset_id.is_empty() {
        return Err(anyhow!("alerts table needs a BQ dataset; create bq first"));
    }

    create_topic_idempotent(ALERTS_TOPIC, config)?;
    tracker.record_alerts_topic(ALERTS_TOPIC.to_string())?;

    create_topic_idempotent(DLQ_TOPIC, config)?;
    tracker.record_dlq_topic(DLQ_TOPIC.to_string())?;

    create_alerts_table(&project, &dataset_id)?;
    tracker.record_alerts_table(ALERTS_TABLE.to_string())?;

    let sub_id = format!("{}-{}",
        ALERTS_SUBSCRIPTION_PREFIX,
        crate::lib::utilities::random_tag(6),
    );
    create_alerts_subscription(&sub_id, ALERTS_TOPIC, &project, &dataset_id)?;
    tracker.record_alerts_subscription(sub_id)?;

    Ok(())
}

fn create_topic_idempotent(name: &str, config: &Config) -> Result<()> {
    // Reuse pubsub::create_named_pubsub_topic; ignore "already exists".
    match create_named_pubsub_topic(name, config) {
        Ok(_) => Ok(()),
        Err(_) => {
            // Idempotency: a second call usually fails because the topic
            // exists. Verify and treat as success.
            let out = Command::new("gcloud")
                .args(["pubsub", "topics", "describe", name])
                .args(config.get_project())
                .output()?;
            if out.status.success() {
                info!("pubsub topic {} already exists", name);
                Ok(())
            } else {
                Err(anyhow!(
                    "failed to create or describe pubsub topic {}: {}",
                    name,
                    String::from_utf8_lossy(&out.stderr)
                ))
            }
        }
    }
}

fn create_alerts_table(project: &str, dataset_id: &str) -> Result<()> {
    let fq = format!("{}:{}.{}", project, dataset_id, ALERTS_TABLE);
    // Schema mirrors Alert dataclass in sigma_beam.alerts. matched_events
    // and tags are JSON for flexibility — Sigma rule shapes vary.
    let schema = "rule_id:STRING,rule_title:STRING,severity:STRING,\
fired_at:TIMESTAMP,window_start:TIMESTAMP,window_end:TIMESTAMP,\
correlation_key:STRING,matched_events:JSON,tags:JSON";
    let out = Command::new("bq")
        .args([
            "mk", "--table",
            "--time_partitioning_type=DAY",
            "--time_partitioning_field=fired_at",
            "--time_partitioning_expiration=2592000", // 30d
            &fq, schema,
        ])
        .output()?;
    if out.status.success() {
        info!("created alerts table {}", fq);
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("already exists") {
        info!("alerts table {} already exists", fq);
        return Ok(());
    }
    Err(anyhow!("failed to create alerts table {}: {}", fq, stderr))
}

fn create_alerts_subscription(
    sub_id: &str, topic: &str, project: &str, dataset_id: &str,
) -> Result<()> {
    let table_arg = format!(
        "--bigquery-table={}.{}.{}", project, dataset_id, ALERTS_TABLE,
    );
    let topic_arg = format!("--topic={}", topic);
    let out = Command::new("gcloud")
        .args([
            "pubsub", "subscriptions", "create", sub_id,
            &topic_arg,
            &table_arg,
            "--use-table-schema",
            // `--write-metadata` is a value-less boolean flag in gcloud; passing
            // `=false` is rejected. Omitting it leaves metadata-writing off (the
            // default), which is what we want — the alert payload maps 1:1 to the
            // table schema via `--use-table-schema`.
            &format!("--project={}", project),
        ])
        .output()?;
    if out.status.success() {
        info!("created alerts BQ subscription {}", sub_id);
        return Ok(());
    }
    Err(anyhow!(
        "alerts subscription create failed: {}",
        String::from_utf8_lossy(&out.stderr),
    ))
}

/// `gsutil rsync` the staged rule YAMLs under `<config>/artifacts/rules/`
/// to `gs://<bucket>/rules/`. Records the resulting gs:// prefix.
pub fn upload_rules(
    config_path: &Path, tracker: &mut Tracker, _config: &Config,
) -> Result<()> {
    let bucket = tracker.resources().bucket_name.clone();
    if bucket.is_empty() {
        return Err(anyhow!("no bucket recorded; can't upload rules"));
    }
    let local = config_path.join("artifacts").join("rules");
    if !local.is_dir() {
        warn!("no staged rules under {}", local.display());
        // Empty ruleset is allowed — pipeline still runs.
        tracker.record_rules_prefix(format!("gs://{}/rules", bucket))?;
        return Ok(());
    }
    let dest = format!("gs://{}/rules", bucket);
    let out = Command::new("gsutil")
        .args(["-m", "rsync", "-r", "-d",
               local.to_str().unwrap(), &dest])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "rules upload failed: {}", String::from_utf8_lossy(&out.stderr),
        ));
    }
    info!("uploaded rules → {}", dest);
    tracker.record_rules_prefix(dest)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Teardown
// ---------------------------------------------------------------------------

pub fn destroy_alerts_subscription(id: &str, config: &Config) -> Result<()> {
    delete_subscription(id, config)
}

pub fn destroy_alerts_topic(id: &str, config: &Config) -> Result<()> {
    delete_topic(id, config)
}

pub fn destroy_dlq_topic(id: &str, config: &Config) -> Result<()> {
    delete_topic(id, config)
}

pub fn destroy_alerts_table(project: &str, dataset_id: &str, table_id: &str) -> Result<()> {
    let fq = format!("{}:{}.{}", project, dataset_id, table_id);
    let out = Command::new("bq")
        .args(["rm", "-f", "--table", &fq])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "alerts table delete failed: {}",
            String::from_utf8_lossy(&out.stderr),
        ));
    }
    Ok(())
}

pub fn destroy_rules_prefix(bucket: &str) -> Result<()> {
    let dest = format!("gs://{}/rules/**", bucket);
    let out = Command::new("gsutil")
        .args(["-m", "rm", "-r", &dest])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("No URLs matched") || stderr.contains("does not exist") {
            return Ok(());
        }
        return Err(anyhow!("rules prefix delete failed: {}", stderr));
    }
    Ok(())
}

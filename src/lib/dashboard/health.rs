use std::process::Command;

use anyhow::{anyhow, Result};
use log::info;

use crate::lib::config::Config;
use crate::lib::resources::Tracker;

/// One entry per component-health alertChart on the dashboard.
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub label: String,
    pub policy_name: String,
}

/// Creates one alert policy per component so the dashboard can render a
/// uniform grid of `alertChart` widgets. For resources with a meaningful
/// runtime metric (Dataflow, sub backlogs) we attach real conditions; for the
/// rest we attach a tautologically-OK threshold so the widget always reads
/// green and signals "deploy succeeded". Returns one entry per component in
/// the order they should appear in the grid.
pub(super) fn create_component_health_policies(
    tracker: &mut Tracker,
    config: &Config,
    metric_name: &str,
) -> Result<Vec<ComponentHealth>> {
    // Snapshot just the fields we need so we can release the &Resources borrow
    // before calling create_health_policy (which takes &mut tracker).
    let (
        crs_instance, dataflow_job, out_topic, bq_sub, df_sub,
        bq_dataset, bq_table, bucket, export_sq
    ) = {
        let r = tracker.resources();
        (
            r.crs_instance.clone(),
            r.dataflow_pipeline_name.clone(),
            r.output_pubsub.topic_id.clone(),
            r.output_pubsub.bq_subscription_id.clone(),
            r.output_pubsub.subscription_id_2.clone(),
            r.biq_query.dataset_id.clone(),
            r.biq_query.table_id.clone(),
            r.bucket_name.clone(),
            r.export_scheduled_query_id.clone(),
        )
    };
    let mut out = Vec::new();

    // ---- Cloud Run: no real runtime signal (scales to 0; metric absent
    // when idle). No-op policy that never trips → always green.
    out.push(ComponentHealth {
        label: "Cloud Run".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver Cloud Run ({})", crs_instance),
            &noop_threshold(
                &format!(r#"resource.type="cloud_run_revision" AND metric.type="run.googleapis.com/container/instance_count" AND resource.label.service_name="{}""#, crs_instance),
                999999.0,
            ))?,
    });

    // ---- Dataflow: real check. is_failed=1 OR metric absent >10min.
    let df_filter = format!(
        r#"resource.type="dataflow_job" AND metric.type="dataflow.googleapis.com/job/is_failed" AND resource.label.job_name="{}""#,
        dataflow_job
    );
    out.push(ComponentHealth {
        label: "Dataflow".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver Dataflow ({})", dataflow_job),
            &format!(
r#"  - displayName: "is_failed > 0"
    conditionThreshold:
      filter: '{f}'
      comparison: COMPARISON_GT
      thresholdValue: 0
      duration: 60s
      aggregations:
        - alignmentPeriod: 60s
          perSeriesAligner: ALIGN_MAX
  - displayName: "job metrics absent"
    conditionAbsent:
      filter: '{f}'
      duration: 600s
      aggregations:
        - alignmentPeriod: 60s
          perSeriesAligner: ALIGN_MAX
"#, f = df_filter))?,
    });

    // ---- Output Pub/Sub topic: idle topics don't emit; no-op.
    out.push(ComponentHealth {
        label: "Output topic".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver Output topic ({})", out_topic),
            &noop_threshold(
                &format!(r#"resource.type="pubsub_topic" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count" AND resource.label.topic_id="{}""#, out_topic),
                1e18,
            ))?,
    });

    // ---- BQ subscription: real backlog check (>10k undelivered = trouble).
    out.push(ComponentHealth {
        label: "BQ subscription".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver BQ subscription ({})", bq_sub),
            &threshold_condition(
                "backlog > 10k",
                &format!(r#"resource.type="pubsub_subscription" AND metric.type="pubsub.googleapis.com/subscription/num_undelivered_messages" AND resource.label.subscription_id="{}""#, bq_sub),
                10000.0, "ALIGN_MAX",
            ))?,
    });

    // ---- Dataflow subscription: same backlog check.
    out.push(ComponentHealth {
        label: "DF subscription".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver Dataflow subscription ({})", df_sub),
            &threshold_condition(
                "backlog > 10k",
                &format!(r#"resource.type="pubsub_subscription" AND metric.type="pubsub.googleapis.com/subscription/num_undelivered_messages" AND resource.label.subscription_id="{}""#, df_sub),
                10000.0, "ALIGN_MAX",
            ))?,
    });

    // ---- BigQuery dataset/table: no actionable runtime signal; no-op.
    out.push(ComponentHealth {
        label: "BigQuery".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver BigQuery ({}.{})", bq_dataset, bq_table),
            &noop_threshold(
                &format!(r#"resource.type="bigquery_dataset" AND metric.type="bigquery.googleapis.com/storage/stored_bytes" AND resource.label.dataset_id="{}""#, bq_dataset),
                1e30,
            ))?,
    });

    // ---- GCS bucket: no-op.
    out.push(ComponentHealth {
        label: "GCS bucket".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver GCS bucket ({})", bucket),
            &noop_threshold(
                &format!(r#"resource.type="gcs_bucket" AND metric.type="storage.googleapis.com/storage/total_bytes" AND resource.label.bucket_name="{}""#, bucket),
                1e30,
            ))?,
    });

    // Cold tier + Export job: removed. The GCS bucket tile above already shows
    // total bucket bytes (GCS metrics are bucket-level, not prefix-level, so a
    // separate cold-tier tile was identical). BQ DTS does not expose a per-config
    // metric usable in alert-policy filters, so no export-job tile is possible.
    let _ = export_sq;

    // ---- Log-based metric: no-op (only emits when detections fire).
    out.push(ComponentHealth {
        label: "Log metric".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver Log metric ({})", metric_name),
            &noop_threshold(
                &format!(r#"resource.type="dataflow_job" AND metric.type="logging.googleapis.com/user/{}""#, metric_name),
                1e18,
            ))?,
    });

    Ok(out)
}

pub(super) fn noop_threshold(filter: &str, value: f64) -> String {
    threshold_condition("no-op (deploy-time check)", filter, value, "ALIGN_MAX")
}

pub(super) fn threshold_condition(label: &str, filter: &str, value: f64, aligner: &str) -> String {
    format!(
r#"  - displayName: "{label}"
    conditionThreshold:
      filter: '{filter}'
      comparison: COMPARISON_GT
      thresholdValue: {value}
      duration: 60s
      aggregations:
        - alignmentPeriod: 60s
          perSeriesAligner: {aligner}
"#)
}

pub(super) fn create_health_policy(
    tracker: &mut Tracker,
    config: &Config,
    display: &str,
    conditions_yaml: &str,
) -> Result<String> {
    let yaml = format!(
"displayName: \"{display}\"\ncombiner: OR\nconditions:\n{conditions_yaml}");
    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), &yaml)?;
    info!("creating health policy: {}", display);
    let out = Command::new("gcloud")
        .args([
            "alpha", "monitoring", "policies", "create",
            &format!("--policy-from-file={}", tmp.path().display()),
            &format!("--project={}", config.project),
            "--format=value(name)",
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "health policy create failed ({}): {}",
            display, String::from_utf8_lossy(&out.stderr)
        ));
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if id.is_empty() {
        return Err(anyhow!("health policy create returned empty name ({})", display));
    }
    tracker.record_alert_policy(id.clone())?;
    Ok(id)
}

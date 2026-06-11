//! SOC-analyst-facing Cloud Monitoring dashboard, plus the log-based metric
//! its per-rule panels depend on. Opt-in via the `dashboard:` section in
//! beaver_config.yaml.

use std::process::Command;

use anyhow::{anyhow, Result};
use log::info;
use serde::{Deserialize, Serialize};

use crate::lib::config::Config;
use crate::lib::resources::Tracker;
use crate::lib::utilities::random_tag;

/// One entry per component-health alertChart on the dashboard.
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub label: String,
    pub policy_name: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct DashboardConfig {
    #[serde(default)]
    pub enabled: bool,
    pub name: String,
}

impl DashboardConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.name.trim().is_empty() {
            return Err(anyhow!("dashboard.enabled=true but dashboard.name is empty"));
        }
        Ok(())
    }
}

/// Creates the log-based metric that extracts `rule_name` from detection
/// events. The dashboard's per-rule panels group on `metric.label.rule_name`,
/// so this must exist before the dashboard does. `gcloud logging metrics
/// create` only supports labelExtractors via `--config-from-file`, so we
/// render the metric definition to a tempfile.
pub fn create_log_metric(tracker: &mut Tracker, config: &Config) -> Result<String> {
    let metric_name = format!("beaver_detection_count_{}", random_tag(6));
    info!("creating log-based metric {}", metric_name);

    // Cloud Logging stores the harness's `logging.warning(json.dumps(...))`
    // call as a JSON-encoded *string* under `jsonPayload.message`, not as a
    // parsed nested object. So we substring-match on the message and regex
    // out the rule_name field.
    let metric_yaml = r#"description: "Beaver SIEM detection events, per rule"
filter: |
  resource.type="dataflow_step" AND jsonPayload.message:"BEAVER_SIEM_MATCH"
metricDescriptor:
  metricKind: DELTA
  valueType: INT64
  labels:
    - key: rule_name
      valueType: STRING
labelExtractors:
  rule_name: REGEXP_EXTRACT(jsonPayload.message, "\"rule_name\":\\s*\"([^\"]+)\"")
"#.to_string();
    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), &metric_yaml)?;

    let out = Command::new("gcloud")
        .args([
            "logging", "metrics", "create", &metric_name,
            &format!("--config-from-file={}", tmp.path().display()),
            &format!("--project={}", config.project),
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "log metric create failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    tracker.record_log_metric(metric_name.clone())?;
    Ok(metric_name)
}

pub fn delete_log_metric(name: &str, project: &str) -> Result<()> {
    info!("deleting log-based metric {}", name);
    let out = Command::new("gcloud")
        .args([
            "logging", "metrics", "delete", name,
            "--quiet",
            &format!("--project={}", project),
        ])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("NOT_FOUND") || stderr.contains("does not exist") {
            return Ok(());
        }
        return Err(anyhow!("log metric delete failed: {}", stderr));
    }
    Ok(())
}

/// Snapshot of all resource identifiers the dashboard needs to scope its
/// panels and health-check scorecards to this specific deploy.
pub struct DashboardContext<'a> {
    pub display: &'a str,
    pub project: &'a str,
    pub region: &'a str,
    pub metric_name: &'a str,
    pub vector_service: &'a str,
    pub dataflow_job: &'a str,
    pub input_subscription: &'a str,
    pub output_topic: &'a str,
    pub bq_subscription: &'a str,
    pub dataflow_subscription: &'a str,
    pub bq_dataset: &'a str,
    pub bq_table: &'a str,
    pub bucket: &'a str,
    pub vector_sa: &'a str,
    pub dataflow_sa: &'a str,
    /// One health policy per provisioned component. Rendered as a uniform
    /// grid of `alertChart` widgets so every resource has the same status UI.
    /// Resources without a meaningful runtime metric get a tautologically-OK
    /// policy — they always show green and indicate "deploy succeeded".
    pub component_health: &'a [ComponentHealth],
    /// Full resource names of `monitoring.AlertPolicy`s beaver provisioned for
    /// this deploy (one per route in `notifications.routes`). Empty when
    /// notifications aren't configured — the alerting row is omitted then.
    pub alert_policies: &'a [String],
}

pub fn create_dashboard(
    tracker: &mut Tracker,
    config: &Config,
    dashboard_cfg: &DashboardConfig,
    metric_name: &str,
    input_subscription: &str,
) -> Result<String> {
    let display = format!("{}-{}", dashboard_cfg.name, random_tag(6));
    info!("creating dashboard {}", display);

    let component_health = create_component_health_policies(tracker, config, metric_name)?;
    let res = tracker.resources();
    let ctx = DashboardContext {
        display: &display,
        project: &config.project,
        region: &config.region,
        metric_name,
        vector_service: &res.crs_instance,
        dataflow_job: &res.dataflow_pipeline_name,
        input_subscription,
        output_topic: &res.output_pubsub.topic_id,
        bq_subscription: &res.output_pubsub.bq_subscription_id,
        dataflow_subscription: &res.output_pubsub.subscription_id_2,
        bq_dataset: &res.biq_query.dataset_id,
        bq_table: &res.biq_query.table_id,
        bucket: &res.bucket_name,
        vector_sa: &res.vector_sa_email,
        dataflow_sa: &res.dataflow_sa_email,
        component_health: &component_health,
        alert_policies: &res.alert_policies,
    };
    let yaml = render_dashboard(&ctx);
    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), &yaml)?;

    let out = Command::new("gcloud")
        .args([
            "monitoring", "dashboards", "create",
            &format!("--config-from-file={}", tmp.path().display()),
            &format!("--project={}", config.project),
            "--format=value(name)",
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "dashboard create failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if id.is_empty() {
        return Err(anyhow!("dashboard create returned empty name"));
    }
    tracker.record_dashboard(id.clone())?;
    Ok(id)
}

/// Creates one alert policy per component so the dashboard can render a
/// uniform grid of `alertChart` widgets. For resources with a meaningful
/// runtime metric (Dataflow, sub backlogs) we attach real conditions; for the
/// rest we attach a tautologically-OK threshold so the widget always reads
/// green and signals "deploy succeeded". Returns one entry per component in
/// the order they should appear in the grid.
fn create_component_health_policies(
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

fn noop_threshold(filter: &str, value: f64) -> String {
    threshold_condition("no-op (deploy-time check)", filter, value, "ALIGN_MAX")
}

fn threshold_condition(label: &str, filter: &str, value: f64, aligner: &str) -> String {
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

fn create_health_policy(
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

pub fn delete_dashboard(id: &str, project: &str) -> Result<()> {
    info!("deleting dashboard {}", id);
    let out = Command::new("gcloud")
        .args([
            "monitoring", "dashboards", "delete", id,
            "--quiet",
            &format!("--project={}", project),
        ])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("NOT_FOUND") || stderr.contains("does not exist") {
            return Ok(());
        }
        return Err(anyhow!("dashboard delete failed: {}", stderr));
    }
    Ok(())
}

/// Renders the dashboard YAML. Layout:
///   Row 0 (text, height 3): markdown list of resource links — one click
///                           jumps straight to each GCP resource's console
///   Row 1 (height 4, full width): live detection-event feed
///   Row 2 (height 4, half + half): top firing rules table, rate chart
///   Row 3 (health, height 3): five component scorecards with red-below thresholds
///   Row 4 (height 4, full width): Dataflow worker logs (warnings + errors)
///   Row 5 (alerting, conditional): notification delivery log panel +
///                                  one alertChart per alert policy. Omitted
///                                  entirely when no policies are configured.
fn render_dashboard(ctx: &DashboardContext) -> String {
    let DashboardContext {
        display,
        project,
        region,
        metric_name,
        vector_service,
        dataflow_job,
        input_subscription,
        output_topic,
        bq_subscription,
        dataflow_subscription,
        bq_dataset,
        bq_table,
        bucket,
        vector_sa,
        dataflow_sa,
        component_health,
        alert_policies,
    } = *ctx;
    let metric_type = format!("logging.googleapis.com/user/{}", metric_name);
    let _ = (input_subscription, vector_sa, dataflow_sa,
             bq_dataset, bq_table, output_topic, bq_subscription,
             dataflow_subscription, bucket, dataflow_job, vector_service);

    // Uniform component-health grid: one alertChart per provisioned component,
    // arranged 4-per-row. The grid sits between the resource-links widget
    // (yPos 0..2) and the detection feed (yPos starts after the grid).
    let cols: usize = 4;
    let cell_w: usize = 12 / cols; // 3
    let cell_h: usize = 3;
    let mut health_tiles = String::new();
    for (i, c) in component_health.iter().enumerate() {
        let x = (i % cols) * cell_w;
        let y = 2 + (i / cols) * cell_h;
        health_tiles.push_str(&format!(
r#"    - xPos: {x}
      yPos: {y}
      width: {w}
      height: {h}
      widget:
        title: "{label}"
        alertChart:
          name: "{policy}"
"#,
            x = x, y = y, w = cell_w, h = cell_h,
            label = c.label, policy = c.policy_name,
        ));
    }
    let health_rows = component_health.len().div_ceil(cols);
    let after_health_y = 2 + health_rows * cell_h;

    // Compact one-line resource links (height 2 widget). Each link deep-jumps
    // to the resource's Cloud Console detail/list page.
    let resource_links = format!(
        "[Cloud Run](https://console.cloud.google.com/run/detail/{region}/{vector_service}/metrics?project={project}) · \
         [Dataflow](https://console.cloud.google.com/dataflow/jobs?project={project}) · \
         [BigQuery](https://console.cloud.google.com/bigquery?project={project}) · \
         [Output topic](https://console.cloud.google.com/cloudpubsub/topic/detail/{output_topic}?project={project}) · \
         [BQ sub](https://console.cloud.google.com/cloudpubsub/subscription/detail/{bq_subscription}?project={project}) · \
         [DF sub](https://console.cloud.google.com/cloudpubsub/subscription/detail/{dataflow_subscription}?project={project}) · \
         [Bucket](https://console.cloud.google.com/storage/browser/{bucket}?project={project}) · \
         [Image](https://console.cloud.google.com/artifacts/docker/{project}/{region}/beaver-images?project={project}) · \
         [IAM](https://console.cloud.google.com/iam-admin/serviceaccounts?project={project}) · \
         [Logs](https://console.cloud.google.com/logs/query?project={project})"
    );

    let indent_md = |s: &str, indent: &str| {
        s.lines()
            .map(|l| format!("{}{}", indent, l))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let links_indented = indent_md(&resource_links, "            ");

    let sigma_beam_y = after_health_y;
    let feed_y = sigma_beam_y + 4;
    let charts_y = feed_y + 4;
    let worker_logs_y = charts_y + 4;
    let notif_y = worker_logs_y + 4;
    let alert_chart_base_y = notif_y + 4;

    let mut yaml = format!(
        r#"displayName: "{display}"
mosaicLayout:
  columns: 12
  tiles:
    - xPos: 0
      yPos: 0
      width: 12
      height: 2
      widget:
        title: "Resources"
        text:
          content: |-
{links_indented}
          format: MARKDOWN
    # ---- Component health grid: one uniform alertChart per resource ----
{health_tiles}    # ---- sigma_beam alerts + DLQ ----
    - xPos: 0
      yPos: {sigma_beam_y}
      width: 6
      height: 4
      widget:
        title: "Alerts / min (sigma_beam)"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: 'resource.type="pubsub_topic" AND resource.labels.topic_id="beaver-alerts" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count"'
                  aggregation:
                    alignmentPeriod: 60s
                    perSeriesAligner: ALIGN_RATE
              plotType: LINE
    - xPos: 6
      yPos: {sigma_beam_y}
      width: 6
      height: 4
      widget:
        title: "DLQ messages / min"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: 'resource.type="pubsub_topic" AND resource.labels.topic_id="beaver-dlq" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count"'
                  aggregation:
                    alignmentPeriod: 60s
                    perSeriesAligner: ALIGN_RATE
              plotType: LINE
    - xPos: 0
      yPos: {feed_y}
      width: 12
      height: 4
      widget:
        title: "Detection events (live feed)"
        logsPanel:
          filter: 'resource.type="dataflow_step" AND jsonPayload.message:"BEAVER_SIEM_MATCH"'
          resourceNames:
            - "projects/{project}"
    - xPos: 0
      yPos: {charts_y}
      width: 6
      height: 4
      widget:
        title: "Top firing rules (1h)"
        timeSeriesTable:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: 'metric.type="{metric_type}"'
                  aggregation:
                    alignmentPeriod: 3600s
                    perSeriesAligner: ALIGN_SUM
                    crossSeriesReducer: REDUCE_SUM
                    groupByFields:
                      - "metric.label.rule_name"
    - xPos: 6
      yPos: {charts_y}
      width: 6
      height: 4
      widget:
        title: "Detection rate by rule (6h)"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: 'metric.type="{metric_type}"'
                  aggregation:
                    alignmentPeriod: 60s
                    perSeriesAligner: ALIGN_RATE
                    crossSeriesReducer: REDUCE_SUM
                    groupByFields:
                      - "metric.label.rule_name"
              plotType: LINE
    - xPos: 0
      yPos: {worker_logs_y}
      width: 12
      height: 4
      widget:
        title: "Dataflow worker logs (warnings + errors)"
        logsPanel:
          filter: 'resource.type="dataflow_step" AND resource.labels.job_name="{dataflow_job}" AND severity>=WARNING'
          resourceNames:
            - "projects/{project}"
"#
    );

    // Alerting / notification linkage. Only rendered when this
    // deploy has alert policies (notifications.yaml was configured).
    if !alert_policies.is_empty() {
        // Full-width logs panel showing notification delivery activity.
        yaml.push_str(&format!(
            r#"    - xPos: 0
      yPos: {notif_y}
      width: 12
      height: 4
      widget:
        title: "Notification delivery (recent)"
        logsPanel:
          filter: 'resource.type="alerting_policy"'
          resourceNames:
            - "projects/{project}"
"#,
            project = project, notif_y = notif_y,
        ));
        // One alertChart per policy — 4 per row, width 3 each, height 3.
        for (i, policy) in alert_policies.iter().enumerate() {
            let x = (i % 4) * 3;
            let y = alert_chart_base_y + (i / 4) * 3;
            // Display name = trailing path segment (the policy ID), avoiding
            // a wall-of-projects-N/alertPolicies prefix in the title bar.
            let short = policy.rsplit('/').next().unwrap_or(policy);
            yaml.push_str(&format!(
                r#"    - xPos: {x}
      yPos: {y}
      width: 3
      height: 3
      widget:
        title: "Alert: {short}"
        alertChart:
          name: "{policy}"
"#
            ));
        }
    }

    yaml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_enabled_without_name() {
        let cfg = DashboardConfig { enabled: true, name: "".into() };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_disabled_without_name() {
        let cfg = DashboardConfig { enabled: false, name: "".into() };
        cfg.validate().unwrap();
    }

    #[test]
    fn validate_accepts_enabled_with_name() {
        let cfg = DashboardConfig { enabled: true, name: "My SIEM".into() };
        cfg.validate().unwrap();
    }

    #[test]
    fn rendered_yaml_includes_metric_and_project() {
        let ctx = DashboardContext {
            display: "Test",
            project: "my-proj",
            region: "us-east1",
            metric_name: "beaver_detection_count_abc",
            vector_service: "beaver-vector-instance-xyz",
            dataflow_job: "beaver-detections-xyz",
            input_subscription: "input-sub",
            output_topic: "beaver_outtopic",
            bq_subscription: "beaver_bqsub",
            dataflow_subscription: "beaver_dfsub",
            bq_dataset: "beaver_dataset",
            bq_table: "table1",
            bucket: "beaver_bkt",
            vector_sa: "vector-sa@example.iam",
            dataflow_sa: "df-sa@example.iam",
            component_health: &[
                ComponentHealth { label: "Cloud Run".into(),
                    policy_name: "projects/my-proj/alertPolicies/h-cr".into() },
                ComponentHealth { label: "Dataflow".into(),
                    policy_name: "projects/my-proj/alertPolicies/h-df".into() },
                ComponentHealth { label: "BigQuery".into(),
                    policy_name: "projects/my-proj/alertPolicies/h-bq".into() },
            ],
            alert_policies: &[],
        };
        let yaml = render_dashboard(&ctx);
        // Sanity: every section that references the parameters should resolve.
        assert!(yaml.contains(r#"displayName: "Test""#));
        assert!(yaml.contains("projects/my-proj"));
        assert!(yaml.contains("logging.googleapis.com/user/beaver_detection_count_abc"));
        // Required widget types present.
        assert!(yaml.contains("logsPanel:"));
        assert!(yaml.contains("timeSeriesTable:"));
        assert!(yaml.contains("xyChart:"));
        // Dataflow worker logs panel — always present.
        assert!(yaml.contains(r#"title: "Dataflow worker logs (warnings + errors)""#));
        assert!(yaml.contains(r#"job_name="beaver-detections-xyz""#));
        // Resource-links text widget — links to each deploy resource.
        assert!(yaml.contains(r#"title: "Resources""#));
        assert!(yaml.contains("format: MARKDOWN"));
        assert!(yaml.contains("https://console.cloud.google.com/run/detail/us-east1/beaver-vector-instance-xyz"));
        // Unified component-health grid: one alertChart per ComponentHealth
        // entry. No scorecards, no markdown checklist — every component looks
        // identical.
        assert!(!yaml.contains("scorecard:"));
        assert!(!yaml.contains(r#"title: "Other components""#));
        assert!(yaml.contains(r#"title: "Cloud Run""#));
        assert!(yaml.contains(r#"title: "Dataflow""#));
        assert!(yaml.contains(r#"title: "BigQuery""#));
        assert!(yaml.contains("projects/my-proj/alertPolicies/h-cr"));
        assert!(yaml.contains("projects/my-proj/alertPolicies/h-df"));
        assert!(yaml.contains("projects/my-proj/alertPolicies/h-bq"));
        // 3 component-health alertCharts when no notification policies present.
        assert_eq!(yaml.matches("alertChart:").count(), 3);
        assert!(!yaml.contains("Notification delivery"));
        // Must be valid YAML.
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    }

    #[test]
    fn rendered_yaml_includes_alert_policies_when_present() {
        let policies = [
            "projects/123/alertPolicies/a-1".to_string(),
            "projects/123/alertPolicies/b-2".to_string(),
        ];
        let ctx = DashboardContext {
            display: "Test",
            project: "my-proj",
            region: "us-east1",
            metric_name: "m",
            vector_service: "v",
            dataflow_job: "d",
            input_subscription: "in",
            output_topic: "t",
            bq_subscription: "s1",
            dataflow_subscription: "s2",
            bq_dataset: "ds",
            bq_table: "tbl",
            bucket: "b",
            vector_sa: "v@x",
            dataflow_sa: "d@x",
            component_health: &[
                ComponentHealth { label: "Cloud Run".into(),
                    policy_name: "projects/123/alertPolicies/h-cr".into() },
            ],
            alert_policies: &policies,
        };
        let yaml = render_dashboard(&ctx);
        // Notification log panel present.
        assert!(yaml.contains(r#"title: "Notification delivery (recent)""#));
        assert!(yaml.contains(r#"filter: 'resource.type="alerting_policy"'"#));
        // 1 component-health alertChart + 2 notification-route alertCharts.
        let count = yaml.matches("alertChart:").count();
        assert_eq!(count, 3, "expected 3 alertChart entries, got {}", count);
        // Full policy name + short suffix both present.
        assert!(yaml.contains(r#"name: "projects/123/alertPolicies/a-1""#));
        assert!(yaml.contains(r#"title: "Alert: a-1""#));
        // Must be valid YAML.
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    }
}

//! SOC-analyst-facing Cloud Monitoring dashboard, plus the log-based metric
//! its per-rule panels depend on. Opt-in via the `dashboard:` section in
//! beaver_config.yaml.

pub mod widgets;
pub mod layout;
pub mod render;
pub mod health;
pub mod panels;

use std::process::Command;

use anyhow::{anyhow, Result};
use log::info;
use serde::{Deserialize, Serialize};

use crate::lib::config::Config;
use crate::lib::resources::Tracker;
use crate::lib::utilities::random_tag;

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

pub fn create_dashboard(
    tracker: &mut Tracker,
    config: &Config,
    dashboard_cfg: &DashboardConfig,
    metric_name: &str,
    input_subscription: &str,
) -> Result<String> {
    let display = format!("{}-{}", dashboard_cfg.name, random_tag(6));
    info!("creating dashboard {}", display);

    let component_health = health::create_component_health_policies(tracker, config, metric_name)?;
    let res = tracker.resources();
    let panel_ctx = panels::PanelContext {
        project: config.project.clone(),
        region: config.region.clone(),
        metric_name: metric_name.to_string(),
        vector_service: res.crs_instance.clone(),
        dataflow_job: res.dataflow_pipeline_name.clone(),
        input_subscription: input_subscription.to_string(),
        output_topic: res.output_pubsub.topic_id.clone(),
        bq_subscription: res.output_pubsub.bq_subscription_id.clone(),
        dataflow_subscription: res.output_pubsub.subscription_id_2.clone(),
        bq_dataset: res.biq_query.dataset_id.clone(),
        bq_table: res.biq_query.table_id.clone(),
        bucket: res.bucket_name.clone(),
        vector_sa: res.vector_sa_email.clone(),
        dataflow_sa: res.dataflow_sa_email.clone(),
        alerts_topic: res.alerts_topic_id.clone(),
        dlq_topic: res.dlq_topic_id.clone(),
    };
    let health_pairs: Vec<(String, String)> = component_health
        .iter()
        .map(|c| (c.label.clone(), c.policy_name.clone()))
        .collect();
    let grid = panels::build_dashboard_grid(&panel_ctx, &health_pairs, &res.alert_policies);
    let yaml = render::render_dashboard_yaml(&display, &grid);
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
        let ctx = panels::PanelContext {
            project: "my-proj".into(),
            region: "us-east1".into(),
            metric_name: "beaver_detection_count_abc".into(),
            vector_service: "beaver-vector-instance-xyz".into(),
            dataflow_job: "beaver-detections-xyz".into(),
            input_subscription: "input-sub".into(),
            output_topic: "beaver_outtopic".into(),
            bq_subscription: "beaver_bqsub".into(),
            dataflow_subscription: "beaver_dfsub".into(),
            bq_dataset: "beaver_dataset".into(),
            bq_table: "table1".into(),
            bucket: "beaver_bkt".into(),
            vector_sa: "vector-sa@example.iam".into(),
            dataflow_sa: "df-sa@example.iam".into(),
            alerts_topic: "beaver-alerts".into(),
            dlq_topic: "beaver-dlq".into(),
        };
        let health = vec![
            ("Cloud Run".to_string(), "projects/my-proj/alertPolicies/h-cr".to_string()),
            ("Dataflow".to_string(), "projects/my-proj/alertPolicies/h-df".to_string()),
            ("BigQuery".to_string(), "projects/my-proj/alertPolicies/h-bq".to_string()),
        ];
        let grid = panels::build_dashboard_grid(&ctx, &health, &[]);
        let yaml = render::render_dashboard_yaml("Test", &grid);

        assert!(yaml.contains("displayName: Test") || yaml.contains(r#"displayName: "Test""#));
        assert!(yaml.contains("my-proj"));
        assert!(yaml.contains("logging.googleapis.com/user/beaver_detection_count_abc"));
        assert!(yaml.contains("logsPanel:"));
        assert!(yaml.contains("timeSeriesTable:"));
        assert!(yaml.contains("xyChart:"));
        assert!(yaml.contains("beaver-detections-xyz"));
        assert!(yaml.contains("alertChart:"));
        assert!(!yaml.contains("Notification delivery"));
        // Verify valid YAML
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    }

    #[test]
    fn rendered_yaml_includes_alert_policies_when_present() {
        let ctx = panels::PanelContext {
            project: "my-proj".into(),
            region: "us-east1".into(),
            metric_name: "m".into(),
            vector_service: "v".into(),
            dataflow_job: "d".into(),
            input_subscription: "in".into(),
            output_topic: "t".into(),
            bq_subscription: "s1".into(),
            dataflow_subscription: "s2".into(),
            bq_dataset: "ds".into(),
            bq_table: "tbl".into(),
            bucket: "b".into(),
            vector_sa: "v@x".into(),
            dataflow_sa: "d@x".into(),
            alerts_topic: "beaver-alerts".into(),
            dlq_topic: "beaver-dlq".into(),
        };
        let health = vec![
            ("Cloud Run".to_string(), "projects/123/alertPolicies/h-cr".to_string()),
        ];
        let policies = vec![
            "projects/123/alertPolicies/a-1".to_string(),
            "projects/123/alertPolicies/b-2".to_string(),
        ];
        let grid = panels::build_dashboard_grid(&ctx, &health, &policies);
        let yaml = render::render_dashboard_yaml("Test", &grid);

        assert!(yaml.contains("Notification delivery"));
        assert!(yaml.contains("Alert: a-1"));
        assert!(yaml.contains("Alert: b-2"));
        // 1 health + 2 notification alert charts = 3
        assert_eq!(yaml.matches("alertChart:").count(), 3);
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    }
}

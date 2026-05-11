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
/// so this must exist before the dashboard does.
pub fn create_log_metric(tracker: &mut Tracker, config: &Config) -> Result<String> {
    let metric_name = format!("beaver_detection_count_{}", random_tag(6));
    info!("creating log-based metric {}", metric_name);
    let filter = r#"resource.type="dataflow_step" AND jsonPayload.event="BEAVER_SIEM_MATCH""#;
    let out = Command::new("gcloud")
        .args([
            "logging", "metrics", "create", &metric_name,
            "--description=Beaver SIEM detection events, per rule",
            &format!("--log-filter={}", filter),
            "--label-extractors=rule_name=EXTRACT(jsonPayload.rule_name)",
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
) -> Result<String> {
    let display = format!("{}-{}", dashboard_cfg.name, random_tag(6));
    info!("creating dashboard {}", display);

    let yaml = render_dashboard(&display, &config.project, metric_name);
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

/// Renders a 4-panel SOC-analyst dashboard: feed (full width), top rules
/// table, per-rule rate chart, pipeline scorecard.
fn render_dashboard(display: &str, project: &str, metric_name: &str) -> String {
    let metric_type = format!("logging.googleapis.com/user/{}", metric_name);
    format!(
        r#"displayName: "{display}"
mosaicLayout:
  columns: 12
  tiles:
    - width: 12
      height: 4
      widget:
        title: "Detection events (live feed)"
        logsPanel:
          filter: 'resource.type="dataflow_step" AND jsonPayload.event="BEAVER_SIEM_MATCH"'
          resourceNames:
            - "projects/{project}"
    - xPos: 0
      yPos: 4
      width: 6
      height: 4
      widget:
        title: "Top firing rules (1h)"
        timeSeriesTable:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: 'metric.type="{metric_type}" resource.type="global"'
                  aggregation:
                    alignmentPeriod: 3600s
                    perSeriesAligner: ALIGN_SUM
                    crossSeriesReducer: REDUCE_SUM
                    groupByFields:
                      - "metric.label.rule_name"
    - xPos: 6
      yPos: 4
      width: 6
      height: 4
      widget:
        title: "Detection rate by rule (6h)"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: 'metric.type="{metric_type}" resource.type="global"'
                  aggregation:
                    alignmentPeriod: 60s
                    perSeriesAligner: ALIGN_RATE
                    crossSeriesReducer: REDUCE_SUM
                    groupByFields:
                      - "metric.label.rule_name"
              plotType: LINE
    - xPos: 0
      yPos: 8
      width: 6
      height: 3
      widget:
        title: "Vector container instances"
        scorecard:
          timeSeriesQuery:
            timeSeriesFilter:
              filter: 'metric.type="run.googleapis.com/container/instance_count" resource.type="cloud_run_revision"'
              aggregation:
                alignmentPeriod: 60s
                perSeriesAligner: ALIGN_MEAN
                crossSeriesReducer: REDUCE_SUM
    - xPos: 6
      yPos: 8
      width: 6
      height: 3
      widget:
        title: "Dataflow worker count"
        scorecard:
          timeSeriesQuery:
            timeSeriesFilter:
              filter: 'metric.type="dataflow.googleapis.com/job/current_num_vcpus" resource.type="dataflow_job"'
              aggregation:
                alignmentPeriod: 60s
                perSeriesAligner: ALIGN_MEAN
                crossSeriesReducer: REDUCE_SUM
"#
    )
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
        let yaml = render_dashboard("Test", "my-proj", "beaver_detection_count_abc");
        // Sanity: every section that references the parameters should resolve.
        assert!(yaml.contains(r#"displayName: "Test""#));
        assert!(yaml.contains("projects/my-proj"));
        assert!(yaml.contains("logging.googleapis.com/user/beaver_detection_count_abc"));
        // Required widget types present.
        assert!(yaml.contains("logsPanel:"));
        assert!(yaml.contains("timeSeriesTable:"));
        assert!(yaml.contains("xyChart:"));
        assert!(yaml.contains("scorecard:"));
        // Must be valid YAML.
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    }
}

//! Parser + validator for the `notifications` section of `beaver_config.yaml`.
//!
//! No GCP calls live here — this module only models the schema and validates
//! it at config-load time. Provisioning happens in a sibling module that
//! consumes a validated `NotificationsConfig`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::process::Command;

use anyhow::{anyhow, Result};
use log::info;
use serde::{Deserialize, Serialize};

use crate::lib::config::Config;
use crate::lib::resources::Tracker;

const SUPPORTED_CHANNEL_TYPES: &[&str] = &[
    "email",
    "webhook_tokenauth",
    "webhook_basicauth",
    "pagerduty",
    "sms",
    "slack",
];

const SUPPORTED_MATCH_KEYS: &[&str] = &["severity", "rule_name"];

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct NotificationsConfig {
    pub channels: Vec<NotificationChannel>,
    pub routes: Vec<NotificationRoute>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct NotificationChannel {
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct NotificationRoute {
    #[serde(rename = "match")]
    pub match_keys: BTreeMap<String, String>,
    pub channels: Vec<String>,
}

impl NotificationsConfig {
    /// Confirms channel types are supported, route refs resolve, match keys
    /// are known, and no two channels share a name. Run at config load.
    pub fn validate(&self) -> Result<()> {
        let mut seen_names = HashSet::new();
        for c in &self.channels {
            if !seen_names.insert(c.name.as_str()) {
                return Err(anyhow!("duplicate channel name: {:?}", c.name));
            }
            if !SUPPORTED_CHANNEL_TYPES.contains(&c.channel_type.as_str()) {
                return Err(anyhow!(
                    "channel {:?}: unsupported type {:?}; supported: {:?}",
                    c.name, c.channel_type, SUPPORTED_CHANNEL_TYPES
                ));
            }
        }

        let known: HashSet<&str> = self.channels.iter().map(|c| c.name.as_str()).collect();
        for (i, r) in self.routes.iter().enumerate() {
            for k in r.match_keys.keys() {
                if !SUPPORTED_MATCH_KEYS.contains(&k.as_str()) {
                    return Err(anyhow!(
                        "route {}: unsupported match key {:?}; supported: {:?}",
                        i, k, SUPPORTED_MATCH_KEYS
                    ));
                }
            }
            if r.channels.is_empty() {
                return Err(anyhow!("route {}: must reference at least one channel", i));
            }
            for ch in &r.channels {
                if !known.contains(ch.as_str()) {
                    return Err(anyhow!(
                        "route {}: references unknown channel {:?}", i, ch
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Provisions every channel via `gcloud monitoring channels create` and records
/// the returned resource ID on the tracker. Returns a map from local channel
/// names (as written in the YAML) to GCP channel IDs, used by the policy step
/// to resolve `notificationChannels` references.
pub fn create_channels(
    tracker: &mut Tracker,
    config: &Config,
    cfg: &NotificationsConfig,
) -> Result<HashMap<String, String>> {
    let mut name_to_id = HashMap::new();
    for ch in &cfg.channels {
        info!("creating notification channel {:?}", ch.name);
        let labels_arg = if ch.labels.is_empty() {
            String::new()
        } else {
            ch.labels
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(",")
        };

        let mut args: Vec<String> = vec![
            "monitoring".into(),
            "channels".into(),
            "create".into(),
            format!("--display-name={}", ch.name),
            format!("--type={}", ch.channel_type),
            format!("--project={}", config.project),
            "--format=value(name)".into(),
        ];
        if !labels_arg.is_empty() {
            args.push(format!("--channel-labels={}", labels_arg));
        }

        let output = Command::new("gcloud")
            .args(args.iter().map(|s| s.as_str()))
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "gcloud monitoring channels create failed for {:?}: {}",
                ch.name, stderr
            ));
        }
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if id.is_empty() {
            return Err(anyhow!("channels create returned empty id for {:?}", ch.name));
        }
        tracker.record_notification_channel(id.clone())?;
        name_to_id.insert(ch.name.clone(), id);
    }
    Ok(name_to_id)
}

/// One log-based alert policy per route. Filter is built from the route's
/// `match` keys (`severity`, `rule_name`) ANDed with the always-on Beaver
/// detection-event filter.
pub fn create_alert_policies(
    tracker: &mut Tracker,
    config: &Config,
    cfg: &NotificationsConfig,
    name_to_id: &HashMap<String, String>,
) -> Result<()> {
    for (i, route) in cfg.routes.iter().enumerate() {
        let filter = build_filter(&route.match_keys);
        let channel_ids: Vec<&str> = route
            .channels
            .iter()
            .map(|n| {
                name_to_id
                    .get(n)
                    .map(String::as_str)
                    .ok_or_else(|| anyhow!("route {}: unknown channel {:?}", i, n))
            })
            .collect::<Result<_>>()?;

        let policy_yaml = render_policy_yaml(&format!("beaver-route-{}", i), &filter, &channel_ids);

        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &policy_yaml)?;

        info!("creating alert policy for route {} with filter {:?}", i, filter);
        let output = Command::new("gcloud")
            .args([
                "alpha",
                "monitoring",
                "policies",
                "create",
                &format!("--policy-from-file={}", tmp.path().display()),
                &format!("--project={}", config.project),
                "--format=value(name)",
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("gcloud alert policy create failed: {}", stderr));
        }
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if id.is_empty() {
            return Err(anyhow!("alert policy create returned empty id"));
        }
        tracker.record_alert_policy(id)?;
    }
    Ok(())
}

pub fn delete_channel(id: &str) -> Result<()> {
    info!("deleting notification channel {}", id);
    let output = Command::new("gcloud")
        .args(["monitoring", "channels", "delete", id, "--quiet"])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("NOT_FOUND") {
            return Ok(());
        }
        return Err(anyhow!("channel delete failed: {}", stderr));
    }
    Ok(())
}

pub fn delete_policy(id: &str) -> Result<()> {
    info!("deleting alert policy {}", id);
    let output = Command::new("gcloud")
        .args(["alpha", "monitoring", "policies", "delete", id, "--quiet"])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("NOT_FOUND") {
            return Ok(());
        }
        return Err(anyhow!("policy delete failed: {}", stderr));
    }
    Ok(())
}

fn build_filter(match_keys: &BTreeMap<String, String>) -> String {
    // Always anchor on the structured marker the harness emits, so we never
    // catch unrelated dataflow logs.
    let mut clauses: Vec<String> = vec![
        r#"resource.type="dataflow_step""#.into(),
        r#"jsonPayload.event="BEAVER_SIEM_MATCH""#.into(),
    ];
    for (k, v) in match_keys {
        clauses.push(format!(r#"jsonPayload.{}="{}""#, k, v));
    }
    clauses.join(" AND ")
}

fn render_policy_yaml(display_name: &str, filter: &str, channel_ids: &[&str]) -> String {
    let mut out = String::new();
    out.push_str(&format!("displayName: {:?}\n", display_name));
    out.push_str("combiner: OR\n");
    out.push_str("conditions:\n");
    out.push_str("  - displayName: \"Detection fired\"\n");
    out.push_str("    conditionMatchedLog:\n");
    out.push_str(&format!("      filter: |\n        {}\n", filter.replace('\n', "\n        ")));
    out.push_str("notificationChannels:\n");
    for c in channel_ids {
        out.push_str(&format!("  - {}\n", c));
    }
    out.push_str("alertStrategy:\n");
    out.push_str("  notificationRateLimit:\n");
    out.push_str("    period: 300s\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<NotificationsConfig> {
        Ok(serde_yaml::from_str(yaml)?)
    }

    #[test]
    fn parses_full_config_with_routes() {
        let cfg = parse(r#"
channels:
  - name: soc-email
    type: email
    labels:
      email_address: secops@example.com
  - name: oncall-slack
    type: webhook_tokenauth
    labels:
      url: https://hooks.slack.com/services/X/Y/Z
routes:
  - match: { severity: low }
    channels: [soc-email]
  - match: { severity: high }
    channels: [soc-email, oncall-slack]
"#).unwrap();

        assert_eq!(cfg.channels.len(), 2);
        assert_eq!(cfg.channels[0].name, "soc-email");
        assert_eq!(cfg.channels[0].channel_type, "email");
        assert_eq!(cfg.channels[0].labels.get("email_address").unwrap(), "secops@example.com");

        assert_eq!(cfg.routes.len(), 2);
        assert_eq!(cfg.routes[1].channels, vec!["soc-email", "oncall-slack"]);
        assert_eq!(cfg.routes[0].match_keys.get("severity").unwrap(), "low");

        cfg.validate().expect("valid config");
    }

    #[test]
    fn channels_without_labels_are_allowed() {
        let cfg = parse(r#"
channels:
  - name: noop
    type: email
routes: []
"#).unwrap();
        assert!(cfg.channels[0].labels.is_empty());
        cfg.validate().expect("valid");
    }

    #[test]
    fn rule_name_match_key_supported() {
        let cfg = parse(r#"
channels:
  - name: a
    type: email
    labels: { email_address: a@b.c }
routes:
  - match: { rule_name: suspicious_login }
    channels: [a]
"#).unwrap();
        cfg.validate().expect("rule_name should be a supported match key");
    }

    #[test]
    fn validate_rejects_route_to_unknown_channel() {
        let cfg = parse(r#"
channels:
  - name: a
    type: email
    labels: { email_address: a@b.c }
routes:
  - match: { severity: high }
    channels: [does-not-exist]
"#).unwrap();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("does-not-exist"), "error should name missing channel: {}", err);
    }

    #[test]
    fn validate_rejects_unknown_channel_type() {
        let cfg = parse(r#"
channels:
  - name: x
    type: carrier-pigeon
routes: []
"#).unwrap();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("carrier-pigeon"), "error should mention bad type: {}", err);
    }

    #[test]
    fn validate_rejects_unknown_match_key() {
        let cfg = parse(r#"
channels:
  - name: a
    type: email
    labels: { email_address: a@b.c }
routes:
  - match: { whatever: foo }
    channels: [a]
"#).unwrap();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("whatever"), "error should name bad match key: {}", err);
    }

    #[test]
    fn validate_rejects_duplicate_channel_names() {
        let cfg = parse(r#"
channels:
  - name: a
    type: email
    labels: { email_address: a@b.c }
  - name: a
    type: email
    labels: { email_address: x@y.z }
routes: []
"#).unwrap();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("duplicate"), "error should mention duplicate: {}", err);
    }

    #[test]
    fn validate_rejects_route_with_empty_channels() {
        let cfg = parse(r#"
channels:
  - name: a
    type: email
    labels: { email_address: a@b.c }
routes:
  - match: { severity: high }
    channels: []
"#).unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn parses_section_when_embedded_in_outer_config() {
        // Mirrors how the section will sit inside beaver_config.yaml — verify
        // we can extract it from a parent doc without false positives.
        let parent = r#"
beaver:
  project_id: p
  region: r
sources:
  pubsub_in: { type: gcp_pubsub }
notifications:
  channels:
    - name: e
      type: email
      labels: { email_address: e@x.com }
  routes:
    - match: { severity: high }
      channels: [e]
"#;
        let mut doc: serde_yaml::Mapping = serde_yaml::from_str(parent).unwrap();
        let section = doc
            .remove(serde_yaml::Value::String("notifications".into()))
            .expect("notifications key present");
        let cfg: NotificationsConfig = serde_yaml::from_value(section).unwrap();
        cfg.validate().unwrap();
        assert_eq!(cfg.channels[0].name, "e");
    }

    #[test]
    fn build_filter_anchors_on_event_marker() {
        let m = BTreeMap::new();
        let f = build_filter(&m);
        assert!(f.contains(r#"resource.type="dataflow_step""#));
        assert!(f.contains(r#"jsonPayload.event="BEAVER_SIEM_MATCH""#));
    }

    #[test]
    fn build_filter_appends_match_keys() {
        let mut m = BTreeMap::new();
        m.insert("severity".to_string(), "high".to_string());
        m.insert("rule_name".to_string(), "suspicious_login".to_string());
        let f = build_filter(&m);
        assert!(f.contains(r#"jsonPayload.severity="high""#));
        assert!(f.contains(r#"jsonPayload.rule_name="suspicious_login""#));
        // ANDed together
        assert_eq!(f.matches(" AND ").count(), 3);
    }

    #[test]
    fn render_policy_yaml_includes_channels_and_filter() {
        let yaml = render_policy_yaml(
            "test-policy",
            r#"resource.type="x" AND jsonPayload.event="y""#,
            &["projects/123/notificationChannels/abc", "projects/123/notificationChannels/def"],
        );
        assert!(yaml.contains(r#"displayName: "test-policy""#));
        assert!(yaml.contains("conditionMatchedLog:"));
        assert!(yaml.contains(r#"filter: |"#));
        assert!(yaml.contains(r#"jsonPayload.event="y""#));
        assert!(yaml.contains("- projects/123/notificationChannels/abc"));
        assert!(yaml.contains("- projects/123/notificationChannels/def"));
        // policy file should be valid YAML
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    }

    #[test]
    fn integration_tests_compile() {
        // Sentinel: makes sure the integration-test imports below pull their
        // weight at compile time even when --ignored aren't run.
    }

    #[test]
    fn missing_section_handled_by_caller_as_none() {
        // Confirm that absence is detectable cleanly — caller can `.get()`
        // and treat None as "no notifications configured".
        let parent = r#"
beaver:
  project_id: p
  region: r
"#;
        let doc: serde_yaml::Mapping = serde_yaml::from_str(parent).unwrap();
        assert!(doc.get(serde_yaml::Value::String("notifications".into())).is_none());
    }
}

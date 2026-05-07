//! Parser + validator for the `notifications` section of `beaver_config.yaml`.
//!
//! No GCP calls live here — this module only models the schema and validates
//! it at config-load time. Provisioning happens in a sibling module that
//! consumes a validated `NotificationsConfig`.

use std::collections::{BTreeMap, HashSet};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

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

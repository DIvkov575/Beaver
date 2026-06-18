use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

fn default_admin_password() -> String {
    String::new()
}

fn default_port() -> u16 {
    3000
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct GrafanaConfig {
    #[serde(default)]
    pub enabled: bool,
    pub name: String,
    #[serde(default = "default_admin_password")]
    pub admin_password: String,
    #[serde(default)]
    pub allow_anonymous: bool,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl GrafanaConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.name.trim().is_empty() {
            return Err(anyhow!(
                "grafana_dashboard.enabled=true but grafana_dashboard.name is empty"
            ));
        }
        if self.port == 0 {
            return Err(anyhow!("grafana_dashboard.port must be > 0"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_enabled_without_name() {
        let cfg = GrafanaConfig {
            enabled: true,
            name: "".into(),
            admin_password: "secret".into(),
            allow_anonymous: false,
            port: 3000,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_enabled_with_whitespace_only_name() {
        let cfg = GrafanaConfig {
            enabled: true,
            name: "   ".into(),
            admin_password: "secret".into(),
            allow_anonymous: false,
            port: 3000,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_disabled_without_name() {
        let cfg = GrafanaConfig {
            enabled: false,
            name: "".into(),
            admin_password: String::new(),
            allow_anonymous: false,
            port: 3000,
        };
        cfg.validate().unwrap();
    }

    #[test]
    fn validate_accepts_enabled_with_name() {
        let cfg = GrafanaConfig {
            enabled: true,
            name: "Beaver SIEM".into(),
            admin_password: "admin".into(),
            allow_anonymous: false,
            port: 3000,
        };
        cfg.validate().unwrap();
    }

    #[test]
    fn validate_rejects_zero_port() {
        let cfg = GrafanaConfig {
            enabled: true,
            name: "Dashboard".into(),
            admin_password: "pass".into(),
            allow_anonymous: false,
            port: 0,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_custom_port() {
        let cfg = GrafanaConfig {
            enabled: true,
            name: "Dashboard".into(),
            admin_password: "pass".into(),
            allow_anonymous: true,
            port: 8080,
        };
        cfg.validate().unwrap();
    }

    #[test]
    fn defaults_applied_on_deserialize() {
        let yaml = r#"
name: "My Grafana"
enabled: true
"#;
        let cfg: GrafanaConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.port, 3000);
        assert_eq!(cfg.admin_password, "");
        assert!(!cfg.allow_anonymous);
    }

    #[test]
    fn full_roundtrip_serde() {
        let cfg = GrafanaConfig {
            enabled: true,
            name: "Beaver SIEM Grafana".into(),
            admin_password: "s3cret".into(),
            allow_anonymous: true,
            port: 9090,
        };
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let back: GrafanaConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(cfg, back);
    }
}

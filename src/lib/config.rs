use std::fs::File;
use std::path::Path;
use anyhow::Result;
use serde_yaml::{Mapping, Value};
use crate::get;
use crate::lib::notifications::NotificationsConfig;

pub struct Config {
    pub region: String,
    pub project: String,
    pub billing_account: Option<String>,
    pub service_account: Option<String>,
    formatted_service_account: Option<String>,
}

impl Config {
    pub fn new(region: &str, project: &str, service_account: Option<&str>) -> Config {
        let service_account_binding: Option<String>;
        let formatted_service_account_binding: Option<String>;

        if service_account.is_none() {
            service_account_binding = None;
            formatted_service_account_binding = None;
        } else {
            service_account_binding = Some(service_account.unwrap().to_string());
            formatted_service_account_binding = Some(format!("--impersonate-service-account={}", service_account.unwrap()));
        }

        Self {
            region: region.to_string(),
            project: project.to_string(),
            billing_account: None,
            service_account: service_account_binding,
            formatted_service_account: formatted_service_account_binding,
        }
    }

    pub fn from_path(path_to_config: &Path) -> Config {
        let beaver_config: Mapping = serde_yaml::from_reader(
            File::open(
                path_to_config.join("beaver_config.yaml")
            ).unwrap()
        ).unwrap();

        let region_binding = get!(beaver_config, "beaver", "region",).as_str().unwrap().to_owned();
        let project_id_binding = get!(beaver_config, "beaver", "project_id",).as_str().unwrap().to_owned();
        let billing_account = beaver_config
            .get(&Value::String("beaver".into()))
            .and_then(|v| v.get(&Value::String("billing_account".into())))
            .and_then(|v| v.as_str())
            .map(String::from);

        Config {
            region: region_binding,
            project: project_id_binding,
            billing_account,
            service_account: None,
            formatted_service_account: None,
        }
    }

    /// Extracts `sources.pubsub_in.subscription` from `beaver_config.yaml`.
    /// This is the subscription Vector reads from — needed for the IAM grant.
    pub fn load_input_subscription(path_to_config: &Path) -> Result<String> {
        let mapping: Mapping = serde_yaml::from_reader(
            File::open(path_to_config.join("beaver_config.yaml"))?
        )?;
        let sources = mapping.get(&Value::String("sources".into()))
            .ok_or_else(|| anyhow::anyhow!("missing sources section"))?;
        let pubsub_in = sources.get(&Value::String("pubsub_in".into()))
            .ok_or_else(|| anyhow::anyhow!("missing sources.pubsub_in"))?;
        let sub = pubsub_in.get(&Value::String("subscription".into()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing sources.pubsub_in.subscription"))?;
        Ok(sub.to_string())
    }

    /// Reads the optional `notifications:` section from `beaver_config.yaml`
    /// and validates it. Returns `None` when the section is missing.
    pub fn load_notifications(path_to_config: &Path) -> Result<Option<NotificationsConfig>> {
        let mapping: Mapping = serde_yaml::from_reader(
            File::open(path_to_config.join("beaver_config.yaml"))?
        )?;
        let key = Value::String("notifications".into());
        match mapping.get(&key) {
            None => Ok(None),
            Some(v) => {
                let cfg: NotificationsConfig = serde_yaml::from_value(v.clone())?;
                cfg.validate()?;
                Ok(Some(cfg))
            }
        }
    }

    pub fn flatten(&self) -> Vec<&str> {
        if self.service_account.is_none() {
            Vec::from(["--region", &self.region, "--project", &self.project, ])
        } else {
            Vec::from(["--region", &self.region, "--project", &self.project, self.formatted_service_account.as_ref().unwrap().as_ref()])
        }
    }
    pub fn get_region(&self) -> Vec<&str> { Vec::from(["--region", &self.region]) }
    pub fn get_project(&self) -> Vec<&str> { Vec::from(["--project", &self.project]) }
    pub fn get_service_account(&self) -> Vec<&str> { Vec::from([self.formatted_service_account.as_ref().unwrap().as_ref()]) }
}
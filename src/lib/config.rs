use std::fs::File;
use std::path::Path;
use serde_yaml::{Mapping, Value};
use crate::get;

pub struct Config {
    pub region: String,
    pub project: String,
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
            service_account: service_account_binding,
            formatted_service_account: formatted_service_account_binding,
        }
    }

    pub fn from_path(path_to_config: &Path) -> Config {
        let beaver_config: Mapping = serde_yaml::from_reader(
            File::open(
                path_to_config.join("../beaver_config/beaver_config.yaml")
            ).unwrap()
        ).unwrap();

        let region_binding = get!(beaver_config, "beaver", "region",).as_str().unwrap().to_owned();
        let project_id_binding = get!(beaver_config, "beaver", "project_id",).as_str().unwrap().to_owned();

        Config {
            region: region_binding,
            project: project_id_binding,
            service_account: None,
            formatted_service_account: None,
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
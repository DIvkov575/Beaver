use std::fs::File;
use std::path::Path;
use serde_yaml::{Mapping, Value};
macro_rules! get {($config: ident,  $($b:literal,)*) => {
    $config$([&Value::String($b.into())])*.clone().as_str().unwrap().to_owned()
};}

pub struct Config {
    pub region: String,
    pub project: String,
    pub service_account: Option<String>,
    formatted_service_account: Option<String>
}

impl Config {
    pub fn new(region: &str, project: &str, service_account: Option<&str>) -> Config {
        if service_account.is_none() {
            Self {region: region.to_string(),
                project: project.to_string(),
                service_account: None,
                formatted_service_account: None
            }
        } else {
            Self {
                region: region.to_string(),
                project: project.to_string(),
                service_account: Some(service_account.unwrap().to_string()),
                formatted_service_account: Some(format!("--impersonate-service-account={}", service_account.unwrap()))
            }
        }
    }

    pub fn from_path(path_to_config: &Path) -> Config {
        let beaver_config: Mapping = serde_yaml::from_reader(
            File::open(
                path_to_config.join("../beaver_config/beaver_config.yaml")
            ).unwrap()
        ).unwrap();

        let region_binding = get!(beaver_config, "region",);
        let project_id_binding = get!(beaver_config, "project_id",);

        Config {
            region: region_binding,
            project: project_id_binding,
            service_account: None,
            formatted_service_account: None
        }
    }

    pub fn flatten(&self) -> Vec<&str> {
        if self.service_account.is_none() {
            Vec::from(["--region", &self.region, "--project", &self.project,  ])
        } else {
            Vec::from(["--region", &self.region, "--project", &self.project,  self.formatted_service_account.as_ref().unwrap().as_ref()])
        }

    }
    pub fn get_region(&self) -> Vec<&str> { Vec::from(["--region", &self.region]) }
    pub fn get_project(&self) -> Vec<&str> { Vec::from(["--project", &self.project]) }
    pub fn get_service_account(&self) -> Vec<&str> { Vec::from([self.formatted_service_account.as_ref().unwrap().as_ref()]) }
}
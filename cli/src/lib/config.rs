pub struct Config<'a> {
    pub region: &'a str,
    pub project: &'a str,
    pub service_account: Option<&'a str>,
    formatted_service_account: Option<String>
}

impl<'a> Config<'a> {
    pub fn new(region: &'a str, project: &'a str, service_account: Option<&'a str>) -> Config<'a> {
        if service_account.is_none() {
            Self {region, project, service_account: None, formatted_service_account: None}
        } else {
            Self {region, project, service_account, formatted_service_account: Some(format!("--impersonate-service-account={}", service_account.unwrap()))}
        }
    }
    // pub fn empty() -> Config<'a> {
    //     Config {region: "", project: "", service_account: None, formatted_service_account: None}
    // }
    //
    // pub fn new_from_file(path: &str ) -> {}
    pub fn flatten(&self) -> Vec<&str> {
        if self.service_account.is_none() {
            Vec::from(["--region", self.region, "--project", self.project,  ])
        } else {
            Vec::from(["--region", self.region, "--project", self.project,  self.formatted_service_account.as_ref().unwrap().as_ref()])
        }

    }
    pub fn get_region(&self) -> Vec<&str> { Vec::from(["--region", self.region]) }
    pub fn get_project(&self) -> Vec<&str> { Vec::from(["--project", self.project]) }
    pub fn get_service_account(&self) -> Vec<&str> { Vec::from([self.formatted_service_account.as_ref().unwrap().as_ref()]) }
}
pub struct Config<'a> {
    pub region: &'a str,
    pub project: &'a str,
}

impl<'a> Config<'a> {
    pub fn new(region: &'a str, project: &'a str) -> Config<'a> {
        Self {region, project }
    }
    pub fn flatten(&self) -> Vec<&str> {
        Vec::from(["--region", self.region.clone(), "--project", self.project.clone() ])

    }
}
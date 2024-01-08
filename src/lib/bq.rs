use std::fmt::format;
use std::process::Command;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::lib::config::Config;
use crate::lib::resources::Resources;


#[derive(Deserialize, Serialize)]
pub struct BqTable {
    pub project_id: String,
    pub dataset_id: String,
    pub table_id: String,

}
impl<'a> BqTable {
    pub fn new (project_id: &'a str, dataset_id: &'a str, table_id: &'a str) -> Self {
        Self {
            project_id: project_id.to_string(),
            dataset_id: dataset_id.to_string(),
            table_id: table_id.to_string()}
    }
    pub fn flatten(&self) -> String {
        format!("{}:{}.{}", self.project_id, self.dataset_id, self.table_id)
    }

    pub fn formatted_flatten(&self) -> String {
        format!("--bigquery-table={}:{}.{}", self.project_id, self.dataset_id, self.table_id)
    }

}

pub fn create_table(resources: &Resources, config: &Config) -> Result<()> {
    let bq_table = resources.biq_query.as_ref().unwrap().borrow();
    let id_binding = format!("{}:{}.{}", config.project, bq_table.dataset_id, bq_table.table_id);
    let args: Vec<&str> = Vec::from([
        "mk",
        "--table",
        id_binding.as_ref(),
        "data: JSON"
    ]);
    Command::new("bq").args(args).spawn().unwrap().wait_with_output()?;
    Ok(())
}

pub fn create_dataset(resources: &Resources, config: &Config) -> Result<()> {
    // roles/bigquery.dataEditor
    // roles/bigquery.dataOwner
    // roles/bigquery.user
    // roles/bigquery.admin

    let bq_table = resources.biq_query.as_ref().unwrap().borrow();
    let id_binding = format!("{}:{}", config.project, bq_table.dataset_id);
    let args: Vec<&str> = Vec::from([
        "mk",
        "--dataset",
        id_binding.as_ref(),
    ]);

    Command::new("bq").args(args).spawn().unwrap().wait_with_output()?;
    Ok(())
}


pub fn check_for_bq() -> Result<()> {
    match Command::new("bq").output() {
        Ok(_) => return Ok(()),
        Err(_) => panic!("Please ensure you have bq (biqquery utility tool installed)"),
    }
}

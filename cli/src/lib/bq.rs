use std::fmt::format;
use std::process::Command;
use anyhow::Result;
use crate::lib::config::Config;
use crate::lib::resources::Resources;

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
    Command::new("bq").args(args).spawn()?;
    Ok(())
}

pub fn create_dataset(resources: &Resources, config: &Config) -> Result<()> {
    // roles/bigquery.dataEditor
    // roles/bigquery.dataOwner
    // roles/bigquery.user
    // roles/bigquery.admin

    // bq --location=LOCATION mk \
    // --dataset \
    // --default_kms_key=KMS_KEY_NAME \
    // --default_partition_expiration=PARTITION_EXPIRATION \
    // --default_table_expiration=TABLE_EXPIRATION \
    // --description="DESCRIPTION" \
    // --label=LABEL_1:VALUE_1 \
    // --label=LABEL_2:VALUE_2 \
    // --max_time_travel_hours=HOURS \
    // --storage_billing_model=BILLING_MODEL \
    // PROJECT_ID:DATASET_ID

    let bq_table = resources.biq_query.as_ref().unwrap().borrow();
    let id_binding = format!("{}:{}", config.project, bq_table.dataset_id);
    let args: Vec<&str> = Vec::from([
        "mk",
        "--dataset",
        id_binding.as_ref(),
    ]);

    Command::new("bq").args(args).spawn()?;
    Ok(())
}


pub fn check_for_bq() -> Result<()> {
    match Command::new("bq").output() {
        Ok(_) => return Ok(()),
        Err(_) => panic!("Please ensure you have bq (biqquery utility tool installed)"),
    }
}

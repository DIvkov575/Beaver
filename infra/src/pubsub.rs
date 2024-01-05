use std::fmt::Display;
use std::process::Command;
use crate::config::Config;
use anyhow::Result;
use rand::distributions::Alphanumeric;
use rand::Rng;

pub struct BqTable<'a> {
    project_id: &'a str,
    dataset_id: &'a str,
    table_id: &'a str,
}
impl<'a> BqTable<'a> {
    pub fn new (project_id: &'a str, dataset_id: &'a str, table_id: &'a str) -> Self {
        Self {project_id, dataset_id, table_id}
    }
    pub fn flatten(&self) -> String {
        format!("{}:{}.{}", self.project_id, self.dataset_id, self.table_id)
    }

    pub fn formatted_flatten(&self) -> String {
        format!("--bigquery-table={}:{}.{}", self.project_id, self.dataset_id, self.table_id)
    }

}



pub fn create_bq_subscription(subscription_id: &str, topic_id: &str, bq_table: &BqTable, config: &Config) -> Result<()> {
    // https://cloud.google.com/pubsub/docs/create-bigquery-subscription
    // gcloud pubsub subscriptions create SUBSCRIPTION_ID \
    // --topic=TOPIC_ID \
    // --bigquery-table=PROJECT_ID:DATASET_ID.TABLE_ID

    let topic_binding = format!("--topic={topic_id}");
    let bq_table_binding = bq_table.formatted_flatten();
    let args: Vec<&str> = Vec::from([
        "pubsub",
        "subscriptions",
        "create",
        subscription_id,
        &topic_binding,
        &bq_table_binding,
    ]);

    Command::new("gcloud").args(args).args(config.get_project()).spawn()?;

    Ok(())
}

pub fn create_pubsub_topic(config: &Config) -> Result<()> {
   let mut random_string: String;
    loop {
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let topic_name= format!("beaver_{random_string}");


        let args: Vec<&str> = Vec::from([
            "pubsub",
            "topics",
            "create",
            topic_name.as_ref()
        ]);

        if Command::new("gcloud").args(args).args(config.get_project()).status().unwrap().success() {
            break
        } else {
            continue
        }
    }
   Ok(())
}

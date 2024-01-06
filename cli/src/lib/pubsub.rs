use std::fmt::Display;
use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
use log::warn;
use rand::distributions::Alphanumeric;
use rand::Rng;
use crate::lib::bq::BqTable;


pub struct PubSub {
    topic_id: String,
    subscriptions: Vec<String>
}

impl PubSub {
    pub fn empty() -> Self {
        Self {
            topic_id: String::new(),
            subscriptions: Vec::new(),
        }
    }
}


pub fn create_bq_subscription(topic_id: &str, bq_table: &BqTable, config: &Config) -> Result<String> {
    // https://cloud.google.com/pubsub/docs/create-bigquery-subscription
    // gcloud pubsub subscriptions create SUBSCRIPTION_ID \
    // --topic=TOPIC_ID \
    // --bigquery-table=PROJECT_ID:DATASET_ID.TABLE_ID
    let mut random_string: String;
    let mut subscription_id;

    loop {
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        subscription_id = format!("beaver_{random_string}");

        let topic_binding = format!("--topic={topic_id}");
        let bq_table_binding = bq_table.formatted_flatten();
        let args: Vec<&str> = Vec::from([
            "pubsub",
            "subscriptions",
            "create",
            &subscription_id,
            &topic_binding,
            &bq_table_binding,
        ]);

        if Command::new("gcloud").args(args).args(config.get_project()).status().unwrap().success() {
            break
        } else {
            continue
        }
    }

    Ok(subscription_id)
}

pub fn create_pubsub_topic(config: &Config) -> Result<String> {
    let mut random_string: String;
    let mut topic_binding: String;
    loop {
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        topic_binding = format!("beaver_{random_string}");


        let args: Vec<&str> = Vec::from([
            "pubsub",
            "topics",
            "create",
            topic_binding.as_ref()
        ]);

        if Command::new("gcloud").args(args).args(config.get_project()).status().unwrap().success() {
            break
        } else {
            continue
        }
    }
   Ok(topic_binding)
}

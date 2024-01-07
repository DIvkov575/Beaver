use std::cmp::Reverse;
use std::fmt::Display;
use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
use log::warn;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Serialize;
use crate::lib::bq::BqTable;
use crate::lib::resources::Resources;


#[derive(Debug, Serialize)]
pub struct PubSub {
    pub topic_id: String,
    pub subscription_id: String
}

impl PubSub {
    pub fn new(topic_id: &str, subscription_id: &str) -> Self {
        Self {
            topic_id: topic_id.to_string(),
            subscription_id: subscription_id.to_string(),
        }
    }
    pub fn empty() -> Self {
        Self {
            topic_id: String::new(),
            subscription_id: String::new(),
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




pub fn create_pubsub_to_bq_subscription(resources: &Resources, config: &Config) -> Result<()> {
    let bq_table = resources.biq_query.as_ref().unwrap().borrow();
    let mut pubsub = resources.output_pubsub.as_ref().unwrap().borrow_mut();

    let topic_id = create_pubsub_topic(&config)?;
    let subscription_id = create_bq_subscription(&topic_id, &bq_table, &config)?;

    pubsub.topic_id = topic_id;
    pubsub.subscription_id = subscription_id;

    Ok(())
}

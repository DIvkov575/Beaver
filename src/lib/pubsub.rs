use std::fmt::Display;
use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use crate::lib::bq::BqTable;
use crate::lib::resources::Resources;


#[derive(Debug,Deserialize, Serialize)]
pub struct PubSub {
    pub topic_id: String,
    pub bq_subscription_id: String, // id only (not fully formatted)
    pub subscription_id_2: String,

}

impl PubSub {
    pub fn new(topic_id: &str, bq_subscription_id: &str, subscription_id_2: &str) -> Self {
        Self {
            topic_id: topic_id.to_string(),
            bq_subscription_id: bq_subscription_id.to_string(),
            subscription_id_2: subscription_id_2.to_string(),
        }
    }
    pub fn empty() -> Self {
        Self {
            topic_id: String::new(),
            bq_subscription_id: String::new(),
            subscription_id_2: String::new(),
        }
    }
}


pub fn create(resources: &mut Resources, config: &Config) -> Result<()> {
    info!("creating pubsub...");
    // creates pubsub topic and subscriptions -> writes to biq query table
    let bq_table= &resources.biq_query;
    let mut pubsub= &mut resources.output_pubsub;

    let topic_id = create_pubsub_topic(&config)?;
    let bq_subscription_id = create_bq_subscription(&topic_id, &bq_table, &config)?;
    let subscription_id_2 = create_subscription(&topic_id, &config)?;

    pubsub.topic_id = topic_id;
    pubsub.bq_subscription_id = bq_subscription_id;
    pubsub.subscription_id_2 = subscription_id_2;

    Ok(())
}


pub fn create_subscription(topic_id: &str, config: &Config) -> Result<String> {
    // tries random subscription names until accepted -> saves subscription
    // return subscription name as string

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
        let args: Vec<&str> = Vec::from([
            "pubsub",
            "subscriptions",
            "create",
            &subscription_id,
            &topic_binding,
        ]);

        let output = Command::new("gcloud").args(args).args(config.get_project()).output()?;

        // TODO: Test in depth -> when does it have stderr
        if output.stderr != [0u8; 0] {
            error!("{:?}", String::from_utf8(output.stderr)?) }

        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            break;
        } else {
            continue;
        }
    }

    Ok(subscription_id)
}
pub fn create_bq_subscription(topic_id: &str, bq_table: &BqTable, config: &Config) -> Result<String> {
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

        let output = Command::new("gcloud").args(args).args(config.get_project()).output()?;

        // TODO: Test in depth -> when does it have stderr
        if output.stderr != [0u8; 0] {
            error!("{:?}", String::from_utf8(output.stderr)?) }

        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            break;
        } else {
            continue;
        }
    }

    Ok(subscription_id)
}


pub fn create_named_pubsub_topic(topic_id: &str, config: &Config) -> Result<()> {
        let args: Vec<&str> = Vec::from([
            "pubsub",
            "topics",
            "create",
            topic_id
        ]);

        Command::new("gcloud").args(args).args(config.get_project()).spawn().unwrap().wait_with_output()?;
    Ok(())
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

        let output = Command::new("gcloud").args(args).args(config.get_project()).output()?;

        // TODO: Test in depth -> when does it have stderr
        if output.stderr != [0u8; 0] {
            error!("{:?}", String::from_utf8(output.stderr)?) }

        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            break;
        } else {
            continue;
        }
    }
   Ok(topic_binding)
}




pub fn create_pubsub_to_bq(resources: &mut Resources, config: &Config) -> Result<()> {
    // creates pubsub topic and subscriptions -> writes to biq query table
    let bq_table= &resources.biq_query;
    let mut pubsub= &mut resources.output_pubsub;

    let topic_id = create_pubsub_topic(&config)?;
    let subscription_id = create_bq_subscription(&topic_id, &bq_table, &config)?;

    pubsub.topic_id = topic_id;
    pubsub.bq_subscription_id = subscription_id;

    Ok(())
}

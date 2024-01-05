
mod crj;
mod config;
mod gcs;
mod bq;
mod pubsub;

use anyhow::Result;
use std::collections::HashMap;
use std::error::Error;
use std::iter::Map;
use std::panic::panic_any;
use std::process::{Command, Output, Stdio};
use crj::*;
use config::*;
use serde_yaml;
use serde_yaml::Mapping;
use crate::bq::*;
use crate::gcs::Bucket;
use crate::pubsub::create_pubsub_topic;

fn main() -> Result<()> {
    let config = Config::new("us-east1", "neon-circle-400322", None);



    // create_bq(&config)?;
    // create_table("table1", "test_sasdf", &config)?;

    create_pubsub_topic(&config)?;

    Ok(())
}



pub fn create_service_account() -> Result<()> {



    Ok(())
}
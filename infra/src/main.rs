mod crj;
mod config;
mod gcs;

use std::collections::HashMap;
use std::error::Error;
use std::iter::Map;
use std::process::{Command, Stdio};
use crj::*;
use config::*;
use serde_yaml;
use serde_yaml::Mapping;
use crate::gcs::Bucket;

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new("us-east1", "neon-circle-400322");

    // Bucket::attempt_create("bucket", &config)?;
    let a = Bucket::create_bucket(&config)?;
    println!("{:?}", a);






    Ok(())
}





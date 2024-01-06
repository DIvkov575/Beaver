use std::error::Error;
use std::fmt::format;
use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
// use uuid

use rand::{distributions::Alphanumeric, Rng}; // 0.8


pub fn create_bucket(config: &Config) -> Result<String> {
    let mut random_string: String;

    loop {
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let binding = format!("gs://beaver_{random_string}");

        let mut flags: Vec<&str> = Vec::from(["--location", config.region, "--project", config.project]);
        let mut args: Vec<&str> = Vec::from(["storage", "buckets", "create", &binding]);


        if Command::new("gcloud").args(args).args(flags).status().unwrap().success() {
            break
        } else {
            continue
        }
    }

    Ok(format!("beaver_{random_string}"))
}

pub fn upload_to_bucket(object_location: &str, bucket_name: &str, config: &Config) -> Result<()> {
    // https://cloud.google.com/storage/docs/uploading-objects#permissions-cli
    // gcloud storage cp OBJECT_LOCATION gs://DESTINATION_BUCKET_NAME/
    let destination_bucket_binding = format!("gs://{bucket_name}");
    let mut args: Vec<&str> = Vec::from([
        "storage",
        "cp",
        &object_location,
        &destination_bucket_binding,
    ]);


    Command::new("gcloud").args(args).spawn()?;

    Ok(())
}
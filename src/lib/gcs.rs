use std::error::Error;
use std::fmt::format;
use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
// use uuid

use rand::{distributions::Alphanumeric, Rng};
use crate::lib::resources::Resources; // 0.8


pub fn create_bucket(resources: &Resources, config: &Config) -> Result<String> {
    let mut random_string: String;

    loop {
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let binding = format!("gs://beaver_{random_string}");

        let mut flags: Vec<&str> = Vec::from(["--location", &config.region, "--project", &config.project]);
        let mut args: Vec<&str> = Vec::from(["storage", "buckets", "create", &binding]);


        if Command::new("gcloud").args(args).args(flags).status().unwrap().success() {
            break
        } else {
            continue
        }
    }

    resources.gcs_bucket.replace(Some(format!("beaver_{}", random_string)));
    Ok(format!("beaver_{random_string}"))
}

pub fn upload_to_bucket(local_location: &str, resources: &Resources, config: &Config) -> Result<()> {
    // https://cloud.google.com/storage/docs/uploading-objects#permissions-cli
    // gcloud storage cp OBJECT_LOCATION gs://DESTINATION_BUCKET_NAME/
    let destination_bucket_binding = format!("gs://{}", resources.gcs_bucket.clone().into_inner().unwrap());
    let args: Vec<&str> = Vec::from([
        "storage",
        "cp",
        &local_location,
        &destination_bucket_binding,
    ]);


    Command::new("gcloud").args(args).spawn().unwrap().wait_with_output()?;

    Ok(())
}
use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
use log::{error, info};

use rand::{distributions::Alphanumeric, Rng};
use crate::lib::resources::Resources;
use crate::lib::utilities::log_output;
use crate::MiscError;



pub fn create_bucket(resources: &mut Resources, config: &Config) -> Result<String> {
    info!("creating bucket...");
    let mut random_string: String;

    let mut ctr = 0u8;
    loop {
        if ctr >= 5 { return Err(MiscError::MaxResourceCreationRetries.into()) }
        ctr += 1;

        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let binding = format!("gs://beaver_{random_string}");

        let flags: Vec<&str> = Vec::from(["--location", &config.region, "--project", &config.project]);
        let args: Vec<&str> = Vec::from(["storage", "buckets", "create", &binding]);
        let output = Command::new("gcloud").args(args).args(flags).output()?;

        // TODO: Test in depth -> when does it have stderr
        if output.stderr != [0u8; 0] { error!("{:?}", String::from_utf8(output.stderr)?) }

        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            break;
        } else {
            continue;
        }

    }

    resources.bucket_name = format!("beaver_{}", random_string);
    Ok(format!("beaver_{random_string}"))
}

pub fn upload_to_bucket(local_location: &str, resources: &Resources, config: &Config) -> Result<()> {
    info!("uploading to bucket...");
    // https://cloud.google.com/storage/docs/uploading-objects#permissions-cli
    // gcloud storage cp OBJECT_LOCATION gs://DESTINATION_BUCKET_NAME/
    let destination_bucket_binding = format!("gs://{}", resources.bucket_name.clone());
    let args: Vec<&str> = Vec::from([
        "storage",
        "cp",
        &local_location,
        &destination_bucket_binding,
    ]);


    let output = Command::new("gcloud").args(args).output()?;
    log_output(&output)?;

    Ok(())
}


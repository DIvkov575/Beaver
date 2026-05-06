use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
use log::{error, info};

use rand::{distributions::Alphanumeric, Rng};
use crate::lib::resources::{Resources, Tracker};
use crate::lib::utilities::log_output;
use crate::MiscError;



pub fn delete_bucket(name: &str) -> Result<()> {
    info!("deleting gcs bucket: {}", name);
    let target = format!("gs://{}", name);
    let output = Command::new("gcloud")
        .args(["storage", "rm", "--recursive", &target])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("bucket delete failed: {}", stderr));
    }
    Ok(())
}

pub fn create_bucket(tracker: &mut Tracker, config: &Config) -> Result<()> {
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

        if output.stderr != [0u8; 0] { error!("{:?}", String::from_utf8(output.stderr)?) }

        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            break;
        } else {
            continue;
        }

    }

    tracker.record_bucket(format!("beaver_{}", random_string))?;
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::lib::resources::Tracker;
    use crate::lib::test_helpers::{bucket_exists, tempdir_resources, test_config};

    #[test]
    #[ignore]
    fn bucket_create_then_delete() {
        let config = test_config();
        let (_dir, mut res) = tempdir_resources();
        let mut tracker = Tracker::new(&mut res);

        create_bucket(&mut tracker, &config).expect("create bucket");
        let name = tracker.resources().bucket_name.clone();
        assert!(!name.is_empty());
        assert!(bucket_exists(&name), "bucket {} should exist", name);

        delete_bucket(&name).expect("delete bucket");
        assert!(!bucket_exists(&name), "bucket {} should be gone", name);
    }
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


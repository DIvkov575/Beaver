use std::fmt::{format, Display};
use std::path::Path;
use std::process::Command;
use crate::lib::config::Config;
use anyhow::Result;
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use crate::lib::bq::BqTable;
use crate::lib::resources::Resources;
use crate::lib::utilities::log_output;
use crate::MiscError;


// gcloud services enable cloudbuild.googleapis.com \
// containerregistry.googleapis.com \
// artifactregistry.googleapis.com

pub fn create_docker_image(path: &Path, resources: &mut Resources, config: &Config) -> Result<()> {
    info!("Building Docker image via Cloud Build...");

    let mut ctr = 0usize;
    loop {
        if ctr >= 3 { return Err(MiscError::MaxResourceCreationRetries.into()); }
        ctr += 1;

        let full_image_name = format!("gcr.io/{}/{}-{}", config.project, "beaver-vector-image", random_tag());
        let path = path.join("artifacts").to_owned();
        let args = vec![
            "builds",
            "submit",
            "--tag",
            &full_image_name,
            path.to_str().unwrap()
        ];

        let output = Command::new("gcloud")
            .args(args.clone())
            .args(config.get_project())
            .output()?;

        if output.stderr != [0u8; 0] { error!("{:?}", String::from_utf8(output.stderr)?) }
        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);

            resources.vector_artifact_url = full_image_name;

            break;
        } else {
            continue;
        }
    }

    Ok(())
}

fn random_tag() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

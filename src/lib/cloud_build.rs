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
use crate::lib::utilities::{log_output, random_tag};
use crate::MiscError;


// Required Google Cloud APIs:
// gcloud services enable cloudbuild.googleapis.com \
// artifactregistry.googleapis.com

pub fn create_docker_image(path: &Path, resources: &mut Resources, config: &Config) -> Result<()> {
    info!("Building Docker image via Cloud Build and saving to Artifact Registry...");

    // Ensure Artifact Registry repository exists
    let repository_name = "beaver-images";
    let repository_format = "docker";
    let repository_location = &config.region;
    
    // Create the repository if it doesn't exist
    let repo_check = Command::new("gcloud")
        .args(["artifacts", "repositories", "describe", repository_name, 
               "--location", repository_location])
        .args(config.get_project())
        .output()?;
    
    if !repo_check.status.success() {
        info!("Creating Artifact Registry repository: {}", repository_name);
        let create_repo = Command::new("gcloud")
            .args(["artifacts", "repositories", "create", repository_name, 
                   "--repository-format", repository_format,
                   "--location", repository_location,
                   "--description", "Repository for Beaver Docker images"])
            .args(config.get_project())
            .output()?;
        
        log_output(&create_repo)?;
        
        if !create_repo.status.success() {
            return Err(anyhow::anyhow!("Failed to create Artifact Registry repository"));
        }
    }

    let mut ctr = 0usize;
    loop {
        if ctr >= 3 { return Err(MiscError::MaxResourceCreationRetries.into()); }
        ctr += 1;

        // Format for Artifact Registry: LOCATION-docker.pkg.dev/PROJECT-ID/REPOSITORY/IMAGE:TAG
        let image_name = format!("beaver-vector-image-{}", random_tag(7));
        let full_image_name = format!("{}-docker.pkg.dev/{}/{}/{}",
            config.region, config.project, repository_name, image_name);
            
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


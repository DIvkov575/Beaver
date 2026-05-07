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
use crate::lib::resources::{Resources, Tracker};
use crate::lib::utilities::{log_output, random_tag};
use crate::MiscError;


// Requires `cloudbuild.googleapis.com` and `artifactregistry.googleapis.com` enabled on the project.

pub fn delete_image(full_url: &str, config: &Config) -> Result<()> {
    info!("deleting artifact image: {}", full_url);
    let output = Command::new("gcloud")
        .args(["artifacts", "docker", "images", "delete", full_url,
               "--delete-tags", "--quiet"])
        .args(config.get_project())
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("image delete failed: {}", stderr));
    }
    Ok(())
}

pub fn delete_repo(name: &str, config: &Config) -> Result<()> {
    info!("deleting artifact repo: {}", name);
    let output = Command::new("gcloud")
        .args(["artifacts", "repositories", "delete", name,
               "--location", &config.region, "--quiet"])
        .args(config.get_project())
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("repo delete failed: {}", stderr));
    }
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::lib::resources::Tracker;
    use crate::lib::test_helpers::{
        artifact_image_exists, artifact_repo_exists, tempdir_resources, test_config,
    };
    use std::fs;
    use tempfile::TempDir;

    /// Builds a trivial image via Cloud Build, verifies it lands in Artifact
    /// Registry, then deletes the image and the repo. Costs ~30s of build time
    /// and a few cents.
    #[test]
    #[ignore]
    fn image_create_then_delete() {
        let config = test_config();

        // Build context: tempdir/artifacts/Dockerfile.
        let ctx = TempDir::new().unwrap();
        let artifacts = ctx.path().join("artifacts");
        fs::create_dir_all(&artifacts).unwrap();
        fs::write(artifacts.join("Dockerfile"), "FROM hello-world\n").unwrap();

        let (_resdir, mut res) = tempdir_resources();
        let mut tracker = Tracker::new(&mut res);

        create_docker_image(ctx.path(), &mut tracker, &config).expect("cloud build");

        let image = tracker.resources().vector_artifact_url.clone();
        let repo = tracker.resources().artifact_registry_repo.clone();
        assert!(!image.is_empty(), "image url should be recorded");
        assert!(!repo.is_empty(), "repo should be recorded");

        assert!(
            artifact_image_exists(&image, &config.project),
            "image {} should exist after build", image
        );
        assert!(
            artifact_repo_exists(&repo, &config.project, &config.region),
            "repo {} should exist after build", repo
        );

        delete_image(&image, &config).expect("delete image");
        assert!(!artifact_image_exists(&image, &config.project), "image leaked");

        delete_repo(&repo, &config).expect("delete repo");
        assert!(!artifact_repo_exists(&repo, &config.project, &config.region), "repo leaked");
    }
}

pub fn create_docker_image(path: &Path, tracker: &mut Tracker, config: &Config) -> Result<()> {
    info!("Building Docker image via Cloud Build and saving to Artifact Registry...");

    let repository_name = "beaver-images";
    let repository_format = "docker";
    let repository_location = &config.region;

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
    tracker.record_artifact_repo(repository_name.to_string())?;

    let mut ctr = 0usize;
    loop {
        if ctr >= 3 { return Err(MiscError::MaxResourceCreationRetries.into()); }
        ctr += 1;

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
            tracker.record_image(full_image_name)?;
            break;
        } else {
            continue;
        }
    }

    Ok(())
}


use anyhow::{anyhow, Result};
use std::process::{Command, ExitStatus, Output};
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::Rng;
use crate::lib::config::Config;
use crate::lib::resources::{Resources, Tracker};
use crate::lib::utilities::log_output;
use crate::{log_func_call, MiscError};


pub fn create_vector(tracker: &mut Tracker, config: &Config) -> Result<()>{
    log_func_call!();
    info!("creating vector...");

    let image_url = tracker.resources().vector_artifact_url.clone();
    let mut random_string: String;
    let mut service_name_binding: String;

    let mut ctr = 0usize;
    loop {
        if ctr >= 5 { return Err(MiscError::MaxResourceCreationRetries.into()) }
        ctr += 1;

        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(4)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        service_name_binding = format!("beaver-vector-instance-{random_string}");

        let args: Vec<&str> =  Vec::from(["run", "deploy", "--no-allow-unauthenticated", &service_name_binding, "--image", &image_url]);

        let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
        log_output(&output)?;
        if output.status.success() { break }

        // Failed deploys leave a non-serving service behind; tear it down before retrying.
        if let Err(e) = delete_crs(&service_name_binding, config) {
            error!("inline cleanup of failed CRS '{}' errored: {}", service_name_binding, e);
        }
    }

    tracker.record_crs_instance(service_name_binding)?;
    mount_gcs_crs(config, tracker.resources())?;

    Ok(())
}
fn create_crs_named(service_name: &str, config: &Config) -> Result<()>{
    log_func_call!();
    let image_url = "docker.io/timberio/vector:latest-alpine";
    let args: Vec<&str> =  Vec::from(["run", "deploy", service_name, "--image", image_url]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;
    Ok(())
}

// Bumps the service to gen2 execution. The bucket-volume mount that should
// happen here is still TODO; flags are scaffolded but commented out until the
// mount path/bucket-name plumbing is wired through.
fn mount_gcs_crs(config: &Config, resources: &Resources) -> Result<()> {
    log_func_call!();
    let crs_instance_name = resources.crs_instance.clone();
    let args = vec!["beta", "run", "services", "update", &crs_instance_name,
                    "--execution-environment", "gen2"];
    let output = Command::new("gcloud").args(args).output()?;
    log_output(&output)?;
    Ok(())
}



pub fn delete_crs(service_name: &str, config: &Config) -> Result<()> {
    log_func_call!();
    info!("deleting Cloud Run service: {}", service_name);

    let args: Vec<&str> = Vec::from(["run", "services", "delete", service_name, "--quiet"]);
    let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
    log_output(&output)?;

    if !output.status.success() {
        return Err(anyhow!("failed to delete Cloud Run service: {}", service_name));
    }
    Ok(())
}

/// Deletes the recorded CRS service (if any) and redeploys a fresh one.
pub fn restart_crs(tracker: &mut Tracker, config: &Config) -> Result<()> {
    log_func_call!();
    info!("restarting Cloud Run service");

    let service_name = tracker.resources().crs_instance.clone();
    if !service_name.is_empty() {
        if let Err(e) = delete_crs(&service_name, config) {
            error!("delete failed, redeploying anyway: {}", e);
        }
    }

    create_vector(tracker, config)?;
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::lib::resources::Tracker;
    use crate::lib::test_helpers::{crs_service_exists, tempdir_resources, test_config};

    /// Deploys a public Cloud Run image to verify create_vector + delete_crs
    /// in isolation from cloud_build (which is tested separately). The image
    /// `gcr.io/cloudrun/hello` is maintained by Google and binds 8080.
    #[test]
    #[ignore]
    fn crs_deploy_then_delete() {
        let config = test_config();
        let (_dir, mut res) = tempdir_resources();
        res.vector_artifact_url = "gcr.io/cloudrun/hello".into();
        let mut tracker = Tracker::new(&mut res);

        create_vector(&mut tracker, &config).expect("crs deploy");
        let name = tracker.resources().crs_instance.clone();
        assert!(!name.is_empty());
        assert!(
            crs_service_exists(&name, &config.project, &config.region),
            "service {} should exist", name
        );

        delete_crs(&name, &config).expect("delete crs");
        assert!(
            !crs_service_exists(&name, &config.project, &config.region),
            "service {} leaked", name
        );
    }
}
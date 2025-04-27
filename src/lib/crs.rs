use anyhow::{anyhow, Result};
use std::process::{Command, ExitStatus, Output};
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::Rng;
use crate::lib::config::Config;
use crate::lib::resources::Resources;
use crate::lib::utilities::log_output;
use crate::{log_func_call, MiscError};


pub fn create_vector(resources: &mut Resources, config: &Config) -> Result<()>{
    log_func_call!();
    info!("creating vector...");

    let mut crs_instance_id = &mut resources.crs_instance;
    let image_url = &resources.vector_artifact_url;
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

        // TODO: double check "--no-allow-unauthenticated"
        let args: Vec<&str> =  Vec::from(["run", "deploy", "--no-allow-unauthenticated", &service_name_binding, "--image", image_url]);

        let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
        log_output(&output)?;
        if output.status.success() { break }
    }

    *crs_instance_id = service_name_binding;
    mount_gcs_crs(&config, &resources)?;

    Ok(())
}
fn create_crs_named(service_name: &str, config: &Config) -> Result<()>{
    log_func_call!();
    let image_url = "docker.io/timberio/vector:latest-alpine";
    let args: Vec<&str> =  Vec::from(["run", "deploy", service_name, "--image", image_url]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;
    Ok(())
}

fn mount_gcs_crs(config: &Config, resources: &Resources) -> Result<()> {
    log_func_call!();

    // gcloud beta run services update SERVICE \
    // --execution-environment gen2 \
    // --add-volume name=VOLUME_NAME,type=cloud-storage,bucket=BUCKET_NAME \
    // --add-volume-mount volume=VOLUME_NAME,mount-path=MOUNT_PATH}


    let crs_instance_name = resources.crs_instance.clone();
    let bucket_name = resources.bucket_name.clone();

    // let volume_name = "vector.yaml";
    // let mount_path = "/etc/vector";
    // let volume = format!("name={},bucket={}", volume_name, &bucket_name);
    // let volume_mount = format!("volume={},mount-path={}", volume_name, mount_path);
    let args = vec!["beta", "run", "services", "update", &crs_instance_name,
                    "--execution-environment", "gen2",
                    // "--add-volume", &volume,
                    // "--add-volume-mount", &volume_mount
    ];

    let output = Command::new("gcloud").args(args).output()?;
    log_output(&output)?;

    Ok(())
}



pub fn delete_crs(service_name: &str, config: &Config) -> Result<()> {
    log_func_call!();
    info!("Deleting Cloud Run service: {}", service_name);
    
    let args: Vec<&str> = Vec::from(["run", "services", "delete", service_name, "--quiet"]);
    let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
    log_output(&output)?;
    
    if !output.status.success() {
        return Err(anyhow!("Failed to delete Cloud Run service: {}", service_name));
    }
    
    info!("Successfully deleted Cloud Run service: {}", service_name);
    Ok(())
}

/// Restarts a CRS instance by cleaning up the existing instance and launching a new one
///
/// # Arguments
///
/// * `resources` - Mutable reference to Resources struct that contains information about cloud resources
/// * `config` - Reference to Config struct that contains configuration information
///
/// # Returns
///
/// * `Result<()>` - Result indicating success or failure
pub fn restart_crs(resources: &mut Resources, config: &Config) -> Result<()> {
    log_func_call!();
    info!("Restarting Cloud Run service instance...");
    
    // Check if there's an existing CRS instance to delete
    let service_name = &resources.crs_instance;
    if !service_name.is_empty() {
        // Delete the existing CRS instance
        info!("Found existing CRS instance '{}', cleaning up...", service_name);
        
        match delete_crs(service_name, config) {
            Ok(_) => info!("Successfully cleaned up existing CRS instance"),
            Err(e) => {
                error!("Failed to delete existing CRS instance: {}", e);
                // Continue with creation even if deletion fails
                info!("Proceeding with creating a new instance anyway...");
            }
        }
    } else {
        info!("No existing CRS instance found in resources");
    }
    
    // Create a new CRS instance
    info!("Creating new CRS instance...");
    create_vector(resources, config)?;
    
    info!("CRS instance successfully restarted: {}", resources.crs_instance);
    
    // Resources are updated in the create_vector function
    // Resources will be saved by the caller
    
    Ok(())
}
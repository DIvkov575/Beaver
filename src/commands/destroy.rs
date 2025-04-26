use anyhow::{anyhow, Result};
use log::{info, error};
use std::path::Path;
use std::process::Command;
use crate::lib::config::Config;
use crate::lib::resources::Resources;
use crate::lib::{bq, crs, dataflow, gcs, pubsub, cron};
use crate::lib::utilities::{check_for_gcloud, validate_config_path};

// pub fn destroy(path_arg: &str) -> Result<()> {
//     info!("=======Destroying Resources======");
//
//     let path = Path::new(path_arg);
//     validate_config_path(&path)?;
//     check_for_gcloud()?;
//
//     // Load config and resources
//     let config = Config::from_path(&path);
//
//     // Try to load resources from file
//     let resources_path = path.join("artifacts/resources.yaml");
//     if !resources_path.exists() {
//         return Err(anyhow!("Resources file not found at {}", resources_path.display()));
//     }
//
//     let resources_content = std::fs::read_to_string(&resources_path)?;
//     let resources: Resources = serde_yaml::from_str(&resources_content)?;
//
//     // Delete Cloud Scheduler job first
//     if !resources.crs_schedule_job_id.is_empty() {
//         info!("Deleting Cloud Scheduler job...");
//         if let Err(e) = cron::delete_scheduler_job(&resources.crs_schedule_job_id, &config) {
//             error!("Failed to delete Cloud Scheduler job: {}", e);
//         }
//     }
//
//     // Delete Dataflow job
//     info!("Deleting Dataflow job...");
//     if let Err(e) = dataflow::delete_job(&resources, &config) {
//         error!("Failed to delete Dataflow job: {}", e);
//     }
//
//     // Delete CRS instance
//     if !resources.crs_instance.is_empty() {
//         info!("Deleting Cloud Run service...");
//         if let Err(e) = crs::delete_crs(&resources.crs_instance, &config) {
//             error!("Failed to delete Cloud Run service: {}", e);
//         }
//     }
//
//     // Delete Pub/Sub topics and subscriptions
//     info!("Deleting Pub/Sub resources...");
//     if let Err(e) = pubsub::delete(&resources, &config) {
//         error!("Failed to delete Pub/Sub resources: {}", e);
//     }
//
//     // Delete BigQuery table
//     info!("Deleting BigQuery table...");
//     if let Err(e) = bq::delete(&resources, &config) {
//         error!("Failed to delete BigQuery table: {}", e);
//     }
//
//     // Delete GCS bucket last (it may contain logs and configs)
//     if !resources.bucket_name.is_empty() {
//         info!("Deleting GCS bucket...");
//         if let Err(e) = gcs::delete_bucket(&resources, &config) {
//             error!("Failed to delete GCS bucket: {}", e);
//         }
//     }
//
//     // Remove the resources.yaml file
//     if let Err(e) = std::fs::remove_file(&resources_path) {
//         error!("Failed to remove resources file: {}", e);
//     }
//
//     info!("Resource destruction completed");
//     Ok(())
// }
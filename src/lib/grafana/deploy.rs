use std::process::Command;

use anyhow::Result;
use log::{error, info};

use crate::lib::cloud_build;
use crate::lib::config::Config;
use crate::lib::grafana::dashboard::GrafanaDashboardBuilder;
use crate::lib::grafana::provisioning;
use crate::lib::grafana::GrafanaConfig;
use crate::lib::resources::Tracker;
use crate::lib::utilities::{log_output, random_tag};
use crate::MiscError;

/// Builds and pushes the Grafana container image via Cloud Build.
///
/// Reuses the shared Artifact Registry repo ("beaver-images") and the common
/// Cloud Build submit helper from `cloud_build`.
pub fn build_and_push_image(
    tracker: &mut Tracker,
    config: &Config,
    grafana_cfg: &GrafanaConfig,
) -> Result<()> {
    info!("Building Grafana Docker image via Cloud Build...");

    let repository_name = cloud_build::ensure_artifact_repo(config)?;

    // Generate the dashboard JSON to bake into the image.
    let dataset_id = &tracker.resources().biq_query.dataset_id;
    let table_id = &tracker.resources().biq_query.table_id;
    let builder = GrafanaDashboardBuilder {
        project: config.project.clone(),
        dataset: dataset_id.clone(),
        table: table_id.clone(),
        title: grafana_cfg.name.clone(),
    };
    let dashboard_json = serde_json::to_string_pretty(&builder.build())?;

    // Write build context to a temporary directory.
    let build_dir = tempfile::tempdir()?;
    let sa_email = tracker.resources().grafana_sa_email.clone();
    provisioning::write_build_context(
        build_dir.path(),
        &dashboard_json,
        grafana_cfg,
        &config.project,
        &sa_email,
    )?;

    let full_image_name = cloud_build::submit_build(
        build_dir.path(),
        "beaver-grafana-image",
        &repository_name,
        config,
    )?;
    tracker.record_grafana_image(full_image_name)?;

    Ok(())
}

/// Deploys the Grafana container to Cloud Run.
///
/// The service runs on port 3000 (Grafana default), with min-instances=0 and
/// max-instances=1 for cost control. The dedicated SA grants BigQuery read.
pub fn create_grafana_service(
    tracker: &mut Tracker,
    config: &Config,
    sa_email: &str,
    allow_anonymous: bool,
) -> Result<()> {
    info!("Deploying Grafana to Cloud Run...");

    let image_url = tracker.resources().grafana_image_url.clone();
    if image_url.is_empty() {
        return Err(anyhow::anyhow!(
            "grafana_image_url is empty — build step must run first"
        ));
    }

    let mut ctr = 0usize;
    let mut service_name: String;
    loop {
        if ctr >= 5 {
            return Err(MiscError::MaxResourceCreationRetries.into());
        }
        ctr += 1;

        service_name = format!("beaver-grafana-{}", random_tag(4));
        let sa_flag = format!("--service-account={}", sa_email);
        let auth_flag = if allow_anonymous {
            "--allow-unauthenticated"
        } else {
            "--no-allow-unauthenticated"
        };
        let mut args: Vec<&str> = vec![
            "run",
            "deploy",
            &service_name,
            "--image",
            &image_url,
            "--port",
            "3000",
            "--min-instances=0",
            "--max-instances=1",
            auth_flag,
        ];
        if !sa_email.is_empty() {
            args.push(&sa_flag);
        }

        let output = Command::new("gcloud")
            .args(args)
            .args(config.flatten())
            .output()?;
        log_output(&output)?;

        if output.status.success() {
            tracker.record_grafana_service(service_name.clone())?;
            break;
        }

        if let Err(e) = delete_grafana_service(&service_name, config) {
            error!(
                "inline cleanup of failed Grafana CRS '{}' errored: {}",
                service_name, e
            );
        }
    }

    Ok(())
}

/// Deletes a Grafana Cloud Run service by name.
pub fn delete_grafana_service(service_name: &str, config: &Config) -> Result<()> {
    info!("deleting Grafana Cloud Run service: {}", service_name);

    let output = Command::new("gcloud")
        .args(["run", "services", "delete", service_name, "--quiet"])
        .args(config.flatten())
        .output()?;
    log_output(&output)?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to delete Grafana Cloud Run service: {}",
            service_name
        ));
    }
    Ok(())
}

/// Deletes the Grafana container image from Artifact Registry.
/// Delegates to the shared `cloud_build::delete_image` helper.
pub fn delete_grafana_image(full_url: &str, config: &Config) -> Result<()> {
    info!("deleting Grafana artifact image: {}", full_url);
    cloud_build::delete_image(full_url, config)
}

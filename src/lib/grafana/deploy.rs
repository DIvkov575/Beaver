use std::process::Command;

use anyhow::Result;
use log::{error, info};

use crate::lib::config::Config;
use crate::lib::grafana::dashboard::GrafanaDashboardBuilder;
use crate::lib::grafana::provisioning;
use crate::lib::grafana::GrafanaConfig;
use crate::lib::resources::Tracker;
use crate::lib::utilities::{log_output, random_tag};
use crate::MiscError;

/// Builds and pushes the Grafana container image via Cloud Build.
///
/// Creates an Artifact Registry repository (if not already present) and
/// submits a Docker build context containing Grafana provisioning files.
pub fn build_and_push_image(
    tracker: &mut Tracker,
    config: &Config,
    grafana_cfg: &GrafanaConfig,
) -> Result<()> {
    info!("Building Grafana Docker image via Cloud Build...");

    let repository_name = "beaver-images";
    let repository_location = &config.region;

    // Ensure the Artifact Registry repo exists (shared with Vector image).
    let repo_check = Command::new("gcloud")
        .args([
            "artifacts",
            "repositories",
            "describe",
            repository_name,
            "--location",
            repository_location,
        ])
        .args(config.get_project())
        .output()?;

    if !repo_check.status.success() {
        info!(
            "Creating Artifact Registry repository: {}",
            repository_name
        );
        let create_repo = Command::new("gcloud")
            .args([
                "artifacts",
                "repositories",
                "create",
                repository_name,
                "--repository-format",
                "docker",
                "--location",
                repository_location,
                "--description",
                "Repository for Beaver Docker images",
            ])
            .args(config.get_project())
            .output()?;

        log_output(&create_repo)?;

        if !create_repo.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to create Artifact Registry repository"
            ));
        }
    }

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

    // Submit Cloud Build with retry loop (same pattern as cloud_build::create_docker_image).
    let mut ctr = 0usize;
    loop {
        if ctr >= 3 {
            return Err(MiscError::MaxResourceCreationRetries.into());
        }
        ctr += 1;

        let image_name = format!("beaver-grafana-image-{}", random_tag(7));
        let full_image_name = format!(
            "{}-docker.pkg.dev/{}/{}/{}",
            config.region, config.project, repository_name, image_name
        );

        let build_path = build_dir.path().to_str()
            .ok_or_else(|| anyhow::anyhow!("temp dir path is not valid UTF-8"))?
            .to_string();
        let output = Command::new("gcloud")
            .args([
                "builds",
                "submit",
                "--tag",
                &full_image_name,
                &build_path,
            ])
            .args(config.get_project())
            .output()?;

        if output.stderr != [0u8; 0] {
            error!("{:?}", String::from_utf8(output.stderr.clone())?);
        }
        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            tracker.record_grafana_image(full_image_name)?;
            break;
        }
    }

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
            if allow_anonymous { "--allow-unauthenticated" } else { "--no-allow-unauthenticated" },
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
            // Record the service name (not URL) — used by destroy to delete.
            tracker.record_grafana_service(service_name.clone())?;
            break;
        }

        // Clean up failed deploy before retrying.
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
pub fn delete_grafana_image(full_url: &str, config: &Config) -> Result<()> {
    info!("deleting Grafana artifact image: {}", full_url);
    let output = Command::new("gcloud")
        .args([
            "artifacts",
            "docker",
            "images",
            "delete",
            full_url,
            "--delete-tags",
            "--quiet",
        ])
        .args(config.get_project())
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Grafana image delete failed: {}",
            stderr
        ));
    }
    Ok(())
}

/// Extracts the Cloud Run service URL from gcloud deploy stdout.
/// Looks for lines matching `https://<service>-<hash>-<region>.a.run.app`.
fn extract_service_url(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("https://") && trimmed.contains(".run.app") {
            return Some(trimmed.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_service_url_finds_url() {
        let stdout = "Deploying container...\nService URL: \nhttps://beaver-grafana-abc-xyz.a.run.app\nDone.\n";
        let url = extract_service_url(stdout);
        assert_eq!(
            url,
            Some("https://beaver-grafana-abc-xyz.a.run.app".to_string())
        );
    }

    #[test]
    fn extract_service_url_returns_none_when_missing() {
        let stdout = "Deploying container...\nDone.\n";
        assert_eq!(extract_service_url(stdout), None);
    }
}

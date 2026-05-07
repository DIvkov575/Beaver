use std::path::Path;
use std::process::Command;
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use rand::distributions::Alphanumeric;
use rand::Rng;
use run_script::ScriptOptions;
use crate::lib::config::Config;
use crate::lib::resources::Tracker;
use crate::lib::utilities::log_output;
use crate::{log_func_call, MiscError};

const TEMPLATE_NAME: &str = "beaver-detection-template";

/// Builds a classic Dataflow template from `detections_gen.py` and uploads it
/// to `gs://<bucket>/templates/beaver-detection-template`. The subscription
/// and project are baked into the template at build time (no ValueProvider).
pub fn create_template(path_to_config: &Path, tracker: &mut Tracker, config: &Config) -> Result<()> {
    log_func_call!();
    info!("creating dataflow template...");

    let res = tracker.resources();
    let bucket = res.bucket_name.clone();
    let subscription = res.output_pubsub.subscription_id_2.clone();

    let detections_path = path_to_config.join("detections");
    let staging = format!("gs://{}/staging", bucket);
    let template_path = format!("gs://{}/templates/{}", bucket, TEMPLATE_NAME);

    let args = vec![
        detections_path.to_str().unwrap().to_string(),
        config.project.clone(),
        subscription,
        staging,
        template_path,
        config.region.clone(),
    ];

    let (code, output, error) = run_script::run(
        r#"
        cd $1
        source venv/bin/activate
        python ../artifacts/detections_gen.py \
            --runner=DataflowRunner \
            --project $2 \
            --subscription $3 \
            --staging_location $4 \
            --template_location $5 \
            --region $6
        "#,
        &args,
        &ScriptOptions::new(),
    )?;

    if code != 0 {
        error!("template build stdout: {}", output);
        error!("template build stderr: {}", error);
        return Err(anyhow!("dataflow template build failed (exit {})", code));
    }
    if !error.is_empty() { warn!("{}", error); }
    info!("template uploaded");
    Ok(())
}

/// Launches the previously uploaded classic template as a streaming Dataflow
/// job, records the job name, and returns. Retries with a fresh random suffix
/// on transient launch failure.
pub fn create_pipeline(tracker: &mut Tracker, config: &Config) -> Result<()> {
    log_func_call!();

    let bucket = tracker.resources().bucket_name.clone();
    let template_path = format!("gs://{}/templates/{}", bucket, TEMPLATE_NAME);

    let mut random_string: String;
    let mut pipeline_name: String;

    let mut ctr = 0usize;
    loop {
        if ctr >= 3 { return Err(MiscError::MaxResourceCreationRetries.into()) }
        ctr += 1;

        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        pipeline_name = format!("beaver-detections-{random_string}");

        info!("launching dataflow job: {}", pipeline_name);

        // Pin worker zone away from `<region>-c` because Dataflow's auto-zone
        // selector keeps picking it and us-east1-c has been stockout-prone.
        // `<region>-b` is the next default; override via BEAVER_DATAFLOW_ZONE.
        let zone = std::env::var("BEAVER_DATAFLOW_ZONE")
            .unwrap_or_else(|_| format!("{}-b", config.region));
        let worker_zone_flag = format!("--worker-zone={}", zone);

        let args = vec![
            "dataflow", "jobs", "run", &pipeline_name,
            "--gcs-location", &template_path,
            "--region", &config.region,
            "--project", &config.project,
            "--enable-streaming-engine",
            &worker_zone_flag,
        ];

        let output = Command::new("gcloud").args(args).output()?;

        if !output.stderr.is_empty() {
            error!("{:?}", String::from_utf8(output.stderr.clone())?);
        }

        if output.status.success() {
            log_output(&output)?;
            tracker.record_dataflow_pipeline(pipeline_name.clone())?;
            break;
        } else {
            warn!("failed to launch pipeline {}, retrying", pipeline_name);
        }
    }

    Ok(())
}

/// Cancels the running Dataflow job. Looks up the job by name in the region
/// to get its actual ID (Dataflow `cancel` requires the ID, not the name).
pub fn delete_job(name: &str, config: &Config) -> Result<()> {
    info!("cancelling dataflow job: {}", name);

    let lookup = Command::new("gcloud")
        .args([
            "dataflow", "jobs", "list",
            "--region", &config.region,
            "--project", &config.project,
            "--filter", &format!("name={}", name),
            "--format=value(JOB_ID)",
            "--status=active",
        ])
        .output()?;
    let job_id = String::from_utf8_lossy(&lookup.stdout).trim().to_string();
    if job_id.is_empty() {
        info!("no active dataflow job named {}; nothing to cancel", name);
        return Ok(());
    }

    let cancel = Command::new("gcloud")
        .args([
            "dataflow", "jobs", "cancel", &job_id,
            "--region", &config.region,
            "--project", &config.project,
        ])
        .output()?;
    if !cancel.status.success() {
        let stderr = String::from_utf8_lossy(&cancel.stderr);
        return Err(anyhow!("dataflow cancel failed: {}", stderr));
    }
    Ok(())
}

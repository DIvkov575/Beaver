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
    let temp = format!("gs://{}/temp", bucket);
    let template_path = format!("gs://{}/templates/{}", bucket, TEMPLATE_NAME);

    let args = vec![
        detections_path.to_str().unwrap().to_string(),
        config.project.clone(),
        subscription,
        staging,
        template_path,
        config.region.clone(),
        temp,
    ];

    // Pin --temp_location to our bucket so the dataflow worker SA, which has
    // storage.objectAdmin only on this bucket, can write temp files. Without
    // this, Dataflow defaults to a managed bucket the SA can't touch.
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
            --region $6 \
            --temp_location $7
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
    let sa_email = tracker.resources().dataflow_sa_email.clone();

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
        // Default to <region>-d because <region>-b has been intermittently
        // stockout-prone for the n1-standard-2 workers Dataflow defaults to.
        // Override via BEAVER_DATAFLOW_ZONE if needed.
        let zone = std::env::var("BEAVER_DATAFLOW_ZONE")
            .unwrap_or_else(|_| format!("{}-d", config.region));
        let worker_zone_flag = format!("--worker-zone={}", zone);
        let sa_flag = format!("--service-account-email={}", sa_email);

        let mut args = vec![
            "dataflow", "jobs", "run", &pipeline_name,
            "--gcs-location", &template_path,
            "--region", &config.region,
            "--project", &config.project,
            "--enable-streaming-engine",
            &worker_zone_flag,
        ];
        if !sa_email.is_empty() {
            args.push(&sa_flag);
        }

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

/// Polls the Dataflow job's `currentState` for up to 5 minutes after submission.
/// Returns `Ok` once state is `JOB_STATE_RUNNING`. Returns `Err` immediately if
/// the job hits a terminal failure state (FAILED, CANCELLED) — and includes
/// the most recent worker errors from Cloud Logging in the error chain so the
/// operator can diagnose without digging through the console.
pub fn wait_for_running(name: &str, config: &Config) -> Result<()> {
    info!("waiting for Dataflow job {} to reach Running state", name);
    let job_id = lookup_job_id(name, config)?;
    let timeout = std::time::Duration::from_secs(300);
    let interval = std::time::Duration::from_secs(15);
    let start = std::time::Instant::now();
    let mut last_state = String::new();
    loop {
        let state = describe_state(&job_id, config)?;
        if state != last_state {
            info!("Dataflow state: {}", state);
            last_state = state.clone();
        }
        match state.as_str() {
            "JOB_STATE_RUNNING" => return Ok(()),
            "JOB_STATE_FAILED" | "JOB_STATE_CANCELLED" | "JOB_STATE_UPDATED" | "JOB_STATE_DRAINED" => {
                let errs = recent_errors(&job_id, config);
                return Err(anyhow!(
                    "Dataflow job {} entered terminal state {} before reaching RUNNING.\n\
                     Recent worker errors:\n{}",
                    name, state, errs
                ));
            }
            _ => {}
        }
        if start.elapsed() > timeout {
            let errs = recent_errors(&job_id, config);
            return Err(anyhow!(
                "Dataflow job {} did not reach RUNNING within {:?} (last state {}).\n\
                 Recent worker errors:\n{}",
                name, timeout, state, errs
            ));
        }
        std::thread::sleep(interval);
    }
}

fn lookup_job_id(name: &str, config: &Config) -> Result<String> {
    let out = Command::new("gcloud")
        .args([
            "dataflow", "jobs", "list",
            "--region", &config.region,
            "--project", &config.project,
            "--filter", &format!("name={}", name),
            "--format=value(JOB_ID)",
        ])
        .output()?;
    let id = String::from_utf8_lossy(&out.stdout)
        .lines().next().unwrap_or("").trim().to_string();
    if id.is_empty() {
        return Err(anyhow!("could not find dataflow job named {}", name));
    }
    Ok(id)
}

fn describe_state(job_id: &str, config: &Config) -> Result<String> {
    let out = Command::new("gcloud")
        .args([
            "dataflow", "jobs", "describe", job_id,
            "--region", &config.region,
            "--project", &config.project,
            "--format=value(currentState)",
        ])
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn recent_errors(job_id: &str, config: &Config) -> String {
    let filter = format!(
        r#"resource.type="dataflow_step" AND resource.labels.job_id="{}" AND severity>=ERROR"#,
        job_id
    );
    let out = Command::new("gcloud")
        .args([
            "logging", "read", &filter,
            "--project", &config.project,
            "--limit=3",
            "--format=value(textPayload)",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            if stdout.trim().is_empty() {
                "  (no errors surfaced in logs yet — check the Dataflow console for status)".into()
            } else {
                stdout.lines().map(|l| format!("  {}", l)).collect::<Vec<_>>().join("\n")
            }
        }
        _ => "  (could not fetch error logs)".into(),
    }
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

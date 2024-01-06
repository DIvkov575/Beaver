// gcloud scheduler jobs create http SCHEDULER_JOB_NAME \
// --location SCHEDULER_REGION \
// --schedule="SCHEDULE" \
// --uri="https://CLOUD_RUN_REGION-run.googleapis.com/apis/run.googleapis.com/v1/namespaces/PROJECT-ID/jobs/JOB-NAME:run" \
// --http-method POST \
// --oauth-service-account-email PROJECT-NUMBER-compute@developer.gserviceaccount.com

use std::fmt::format;
use std::process::Command;
use anyhow::Result;
use crate::lib::config::Config;

pub fn create_scheduler(schedule: &str, job_name: &str, config: &Config) -> Result<()> {
    // https://cloud.google.com/run/docs/execute/jobs-on-schedule

    let uri_binding =   format!("--uri=https://{}-run.googleapis.com/apis/run.googleapis.com/v1/namespaces/{}/jobs/{job_name}:run", {config.region}, {config.project});
    let location_binding = format!("--location={}", config.region);

    let args: Vec<&str> =  Vec::from([
        "scheduler", "jobs", "create", "http", "job",
        "--http-method", "POST",
        "--schedule", schedule,
        &location_binding,
        &uri_binding,
     ]);
    Command::new("gcloud").args(args).status()?;

    Ok(())
}
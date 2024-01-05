// gcloud scheduler jobs create http SCHEDULER_JOB_NAME \
// --location SCHEDULER_REGION \
// --schedule="SCHEDULE" \
// --uri="https://CLOUD_RUN_REGION-run.googleapis.com/apis/run.googleapis.com/v1/namespaces/PROJECT-ID/jobs/JOB-NAME:run" \
// --http-method POST \
// --oauth-service-account-email PROJECT-NUMBER-compute@developer.gserviceaccount.com

use std::process::Command;
use anyhow::Result;

pub fn create_scheduler(config: &Config) -> Result<()> {
    // https://cloud.google.com/run/docs/execute/jobs-on-schedule
    let args: Vec<&str> =  Vec::from([
        "scheduler", "jobs", "create", "http"
       ]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;

    Ok(())
}
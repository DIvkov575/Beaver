// gcloud scheduler jobs create http SCHEDULER_JOB_NAME \
// --location SCHEDULER_REGION \
// --schedule="SCHEDULE" \
// --uri="https://CLOUD_RUN_REGION-run.googleapis.com/apis/run.googleapis.com/v1/namespaces/PROJECT-ID/jobs/JOB-NAME:run" \
// --http-method POST \
// --oauth-service-account-email PROJECT-NUMBER-compute@developer.gserviceaccount.com

use std::process::Command;
use anyhow::Result;
use crate::lib::config::Config;
use crate::lib::resources::Resources;

pub fn create_scheduler(schedule: &str, resources: &Resources, config: &Config) -> Result<()> {
    // https://cloud.google.com/run/docs/execute/jobs-on-schedule

    let location_binding = format!("--location={}", config.region);
    let uri_binding =
        format!("--uri=https://{}-run.googleapis.com/apis/run.googleapis.com/v1/namespaces/{}/jobs/{}:run",
            config.region,
            config.project,
            resources.crj_instance.borrow()
        );

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


pub fn scheduler_update() -> Result<()> {
    // gcloud RESOURCE_TYPE add-iam-policy-binding RESOURCE_ID \
    // --member=PRINCIPAL --role=ROLE


    Ok(())
}

use std::process::Command;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use log::{info, error};
use crate::lib::config::Config;
use anyhow::Result;
use crate::lib::resources::Resources;
use crate::MiscError;
use chrono;

pub fn create_crs_restart_schedule(resources: &mut Resources, config: &Config) -> Result<String> {
    info!("creating Cloud Scheduler job...");
    let mut random_string: String;

    let mut ctr = 0u8;
    loop {
        if ctr >= 5 { return Err(MiscError::MaxResourceCreationRetries.into()) }
        ctr += 1;

        random_string = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();

        let job_name = format!("beaver-cron-{}", random_string);
        let target_uri = format!("https://run.googleapis.com/apis/serving.knative.dev/v1/namespaces/{}/services/{}", config.project, resources.crs_instance);
        let time = format!(r#"{{"metadata":{{"annotations":{{"restart-timestamp":"{}"}}}}}}"#, chrono::Utc::now().timestamp());

        let args: Vec<&str> = vec![
            "scheduler", "jobs", "create", "http", &job_name,
            "--schedule", "0 * * * *",
            "--http-method", "POST",
            "--uri", &target_uri,
            "--message-body", &time,
            // "--oidc-service-account-email", &config.scheduler_sa_email,
            // "--oidc-token-audience", "https://run.googleapis.com/",
            "--project", &config.project,
            "--location", &config.region,
        ];

        let output = Command::new("gcloud")
            .args(args)
            .output()?;

        if output.stderr != [0u8; 0] {
            error!("{:?}", String::from_utf8(output.stderr)?)
        }

        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            break;
        } else {
            continue;
        }
    }

    resources.scheduler_job_name = format!("beaver-cron-{}", random_string);
    resources.crs_schedule_job_id = resources.scheduler_job_name.clone();
    Ok(resources.scheduler_job_name.clone())
}

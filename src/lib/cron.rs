use std::process::Command;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use log::{info, error};
use crate::lib::config::Config;
use anyhow::Result;
use crate::lib::resources::{Resources, Tracker};
use crate::MiscError;
use chrono;

pub fn create_crs_restart_schedule(tracker: &mut Tracker, config: &Config) -> Result<()> {
    info!("creating Cloud Scheduler job...");
    let mut random_string: String;
    let crs_instance = tracker.resources().crs_instance.clone();

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
        let target_uri = format!("https://run.googleapis.com/apis/serving.knative.dev/v1/namespaces/{}/services/{}", config.project, crs_instance);
        let time = format!(r#"{{"metadata":{{"annotations":{{"restart-timestamp":"{}"}}}}}}"#, chrono::Utc::now().timestamp());

        let args: Vec<&str> = vec![
            "scheduler", "jobs", "create", "http", &job_name,
            "--schedule", "0 * * * *",
            "--http-method", "POST",
            "--uri", &target_uri,
            "--message-body", &time,
            "--project", &config.project,
            "--location", &config.region,
        ];

        let output = Command::new("gcloud").args(args).output()?;

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

    tracker.record_scheduler_job(format!("beaver-cron-{}", random_string))?;
    Ok(())
}

pub fn delete_scheduler_job(name: &str, config: &Config) -> Result<()> {
    info!("deleting scheduler job: {}", name);
    // Cloud Scheduler returns ABORTED ("sync mutate calls cannot be queued") if
    // delete arrives too soon after create. Retry with a short backoff before
    // surfacing the error.
    let mut last_stderr = String::new();
    for attempt in 0..5 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
        }
        let output = std::process::Command::new("gcloud")
            .args(["scheduler", "jobs", "delete", name,
                   "--location", &config.region,
                   "--project", &config.project,
                   "--quiet"])
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        last_stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !last_stderr.contains("ABORTED") {
            break;
        }
        info!("scheduler delete ABORTED, retrying (attempt {})", attempt + 1);
    }
    Err(anyhow::anyhow!("scheduler delete failed: {}", last_stderr))
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::lib::resources::Tracker;
    use crate::lib::test_helpers::{scheduler_job_exists, tempdir_resources, test_config};

    #[test]
    #[ignore]
    fn scheduler_job_create_then_delete() {
        let config = test_config();
        let (_dir, mut res) = tempdir_resources();
        // create_crs_restart_schedule reads crs_instance from the tracker; the URL
        // doesn't have to point at a real service for the scheduler create to succeed.
        res.crs_instance = "fake-target".into();
        let mut tracker = Tracker::new(&mut res);

        create_crs_restart_schedule(&mut tracker, &config).expect("create scheduler job");
        let name = tracker.resources().scheduler_job_name.clone();
        assert!(!name.is_empty());
        assert!(scheduler_job_exists(&name, &config.project, &config.region));

        delete_scheduler_job(&name, &config).expect("delete scheduler job");
        assert!(!scheduler_job_exists(&name, &config.project, &config.region));
    }
}

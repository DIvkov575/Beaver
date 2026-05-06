//! Helpers shared by `#[ignore]` integration tests that hit real GCP.
//!
//! These tests are opt-in: run with `cargo test -- --ignored`. They expect
//! `gcloud` to be authenticated against a project where Beaver resources can
//! be created and destroyed. Defaults match the project we've been testing
//! against; override via env vars.

#![cfg(test)]
#![allow(dead_code)]

use crate::lib::config::Config;
use crate::lib::resources::Resources;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Project to run integration tests against. Override with `BEAVER_TEST_PROJECT`.
pub fn test_project() -> String {
    std::env::var("BEAVER_TEST_PROJECT").unwrap_or_else(|_| "neon-circle-400322".to_string())
}

/// Region for integration tests. Override with `BEAVER_TEST_REGION`.
pub fn test_region() -> String {
    std::env::var("BEAVER_TEST_REGION").unwrap_or_else(|_| "us-east1".to_string())
}

pub fn test_config() -> Config {
    Config::new(&test_region(), &test_project(), None)
}

/// Build a `Resources` rooted at a fresh tempdir so `Tracker::persist` works.
/// Returns the dir guard so it stays alive for the duration of the test.
pub fn tempdir_resources() -> (TempDir, Resources) {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("artifacts")).unwrap();
    let res = Resources::empty(&test_config(), dir.path());
    (dir, res)
}

fn gcloud_describe_succeeds(args: &[&str]) -> bool {
    let out = Command::new("gcloud")
        .args(args)
        .output()
        .expect("gcloud failed to spawn");
    out.status.success()
}

pub fn bq_dataset_exists(dataset_id: &str, project: &str) -> bool {
    let out = Command::new("bq")
        .args(["show", "--format=none", &format!("{}:{}", project, dataset_id)])
        .output()
        .expect("bq failed to spawn");
    out.status.success()
}

pub fn pubsub_topic_exists(id: &str, project: &str) -> bool {
    gcloud_describe_succeeds(&["pubsub", "topics", "describe", id, "--project", project])
}

pub fn pubsub_subscription_exists(id: &str, project: &str) -> bool {
    gcloud_describe_succeeds(&["pubsub", "subscriptions", "describe", id, "--project", project])
}

pub fn bucket_exists(name: &str) -> bool {
    let out = Command::new("gcloud")
        .args(["storage", "buckets", "describe", &format!("gs://{}", name)])
        .output()
        .expect("gcloud failed");
    out.status.success()
}

pub fn artifact_repo_exists(name: &str, project: &str, region: &str) -> bool {
    gcloud_describe_succeeds(&[
        "artifacts", "repositories", "describe", name,
        "--location", region, "--project", project,
    ])
}

pub fn artifact_image_exists(full_url: &str, project: &str) -> bool {
    let out = Command::new("gcloud")
        .args(["artifacts", "docker", "images", "describe", full_url,
               "--project", project])
        .output()
        .expect("gcloud failed");
    out.status.success()
}

pub fn crs_service_exists(name: &str, project: &str, region: &str) -> bool {
    gcloud_describe_succeeds(&[
        "run", "services", "describe", name,
        "--region", region, "--project", project,
    ])
}

pub fn scheduler_job_exists(name: &str, project: &str, region: &str) -> bool {
    gcloud_describe_succeeds(&[
        "scheduler", "jobs", "describe", name,
        "--location", region, "--project", project,
    ])
}

/// Count beaver-prefixed resources of a given kind currently in the project.
/// Used by the E2E test to assert nothing is left over.
pub fn count_with_prefix(kind: ResourceKind, prefix: &str, project: &str, region: &str) -> usize {
    let args: Vec<String> = match kind {
        ResourceKind::CrsService => vec![
            "run".into(), "services".into(), "list".into(),
            "--region".into(), region.into(),
            "--project".into(), project.into(),
            "--format=value(metadata.name)".into(),
        ],
        ResourceKind::PubsubTopic => vec![
            "pubsub".into(), "topics".into(), "list".into(),
            "--project".into(), project.into(),
            "--format=value(name)".into(),
        ],
        ResourceKind::PubsubSubscription => vec![
            "pubsub".into(), "subscriptions".into(), "list".into(),
            "--project".into(), project.into(),
            "--format=value(name)".into(),
        ],
        ResourceKind::Bucket => vec![
            "storage".into(), "buckets".into(), "list".into(),
            "--project".into(), project.into(),
            "--format=value(name)".into(),
        ],
        ResourceKind::SchedulerJob => vec![
            "scheduler".into(), "jobs".into(), "list".into(),
            "--location".into(), region.into(),
            "--project".into(), project.into(),
            "--format=value(name)".into(),
        ],
    };
    let out = Command::new("gcloud")
        .args(args.iter().map(|s| s.as_str()))
        .output()
        .expect("gcloud failed");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| line.contains(prefix))
        .count()
}

#[derive(Copy, Clone)]
pub enum ResourceKind {
    CrsService,
    PubsubTopic,
    PubsubSubscription,
    Bucket,
    SchedulerJob,
}

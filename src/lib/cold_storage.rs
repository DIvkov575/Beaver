//! Cold tier: parquet on GCS exposed via a BigLake external table, plus a
//! scheduled query that rolls partitions out of the hot BQ table daily.

use std::process::Command;
use anyhow::{anyhow, Result};
use log::info;
use rand::distributions::Alphanumeric;
use rand::Rng;
use tempfile::NamedTempFile;
use crate::lib::config::Config;
use crate::lib::resources::Tracker;

pub(crate) const HOT_RETENTION_DAYS: u32 = 14;
pub(crate) const EXPORT_AGE_DAYS: u32 = 13;
pub(crate) const EXPORT_SCHEDULE: &str = "every 24 hours";
pub(crate) const PARQUET_PREFIX: &str = "parquet";

const LIFECYCLE_JSON: &str = include_str!("schemas/lifecycle.json");

pub fn create(tracker: &mut Tracker, config: &Config) -> Result<()> {
    info!("creating cold tier...");

    let connection_id = create_biglake_connection(config)?;
    tracker.record_biglake_connection(connection_id.clone())?;

    apply_lifecycle_and_grant(tracker, config, &connection_id)?;
    tracker.record_cold_bucket_prefix(PARQUET_PREFIX.into())?;

    seed_sentinel_parquet(tracker, config)?;

    let cold_table = create_cold_table(tracker, config, &connection_id)?;
    tracker.record_cold_table(cold_table)?;

    let view = create_events_view(tracker, config)?;
    tracker.record_events_view(view)?;

    let sq = create_export_scheduled_query(tracker, config)?;
    tracker.record_export_scheduled_query(sq)?;

    Ok(())
}

pub fn destroy_scheduled_query(id: &str, project: &str) -> Result<()> {
    let out = Command::new("bq")
        .args(["rm", "-f", "--transfer_config", id, "--project_id", project])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq rm transfer_config failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

pub fn destroy_view(dataset: &str, view: &str, project: &str) -> Result<()> {
    let fq = format!("{}:{}.{}", project, dataset, view);
    let out = Command::new("bq").args(["rm", "-f", "-t", &fq]).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq rm view failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

pub fn destroy_cold_table(dataset: &str, table: &str, project: &str) -> Result<()> {
    let fq = format!("{}:{}.{}", project, dataset, table);
    let out = Command::new("bq").args(["rm", "-f", "-t", &fq]).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq rm cold table failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

pub fn destroy_connection(connection_id: &str, project: &str, region: &str) -> Result<()> {
    let out = Command::new("bq")
        .args([
            "rm", "-f", "--connection",
            "--project_id", project,
            "--location", region,
            connection_id,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq rm connection failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

// Stubs filled in by Tasks 4–8.
fn create_biglake_connection(_config: &Config) -> Result<String> {
    Err(anyhow!("Task 4"))
}
fn apply_lifecycle_and_grant(_t: &mut Tracker, _c: &Config, _conn: &str) -> Result<()> {
    let _ = LIFECYCLE_JSON; // referenced by Task 5
    let _: Option<NamedTempFile> = None; // referenced by Task 5
    Err(anyhow!("Task 5"))
}
fn seed_sentinel_parquet(_t: &mut Tracker, _c: &Config) -> Result<()> {
    Err(anyhow!("Task 6"))
}
fn create_cold_table(_t: &mut Tracker, _c: &Config, _conn: &str) -> Result<String> {
    Err(anyhow!("Task 6"))
}
fn create_events_view(_t: &mut Tracker, _c: &Config) -> Result<String> {
    Err(anyhow!("Task 7"))
}
fn create_export_scheduled_query(_t: &mut Tracker, _c: &Config) -> Result<String> {
    Err(anyhow!("Task 8"))
}

#[allow(dead_code)]
fn _suppress_unused_warnings() {
    let _ = (
        HOT_RETENTION_DAYS,
        EXPORT_AGE_DAYS,
        EXPORT_SCHEDULE,
        Alphanumeric,
    );
    let _: Option<&dyn Fn() -> String> = None;
    let _ = std::iter::empty::<rand::rngs::ThreadRng>();
}

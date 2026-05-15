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

pub(crate) fn build_biglake_connection_args(id: &str, project: &str, region: &str) -> Vec<String> {
    vec![
        "mk".into(),
        "--connection".into(),
        "--connection_type=CLOUD_RESOURCE".into(),
        format!("--project_id={}", project),
        format!("--location={}", region),
        id.into(),
    ]
}

fn create_biglake_connection(config: &Config) -> Result<String> {
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let id = format!("beaver_biglake_{}", suffix);
    let args = build_biglake_connection_args(&id, &config.project, &config.region);
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = Command::new("bq").args(&argrefs).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq mk --connection failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    info!("biglake connection created: {}", id);
    Ok(id)
}

pub(crate) fn connection_service_account(id: &str, project: &str, region: &str) -> Result<String> {
    let out = Command::new("bq")
        .args([
            "show", "--connection",
            "--project_id", project,
            "--location", region,
            "--format=value(cloudResource.serviceAccountId)",
            id,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq show --connection failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let sa = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sa.is_empty() {
        return Err(anyhow!("connection {} has no service account", id));
    }
    Ok(sa)
}
pub(crate) fn build_grant_object_viewer_args(bucket: &str, sa: &str) -> Vec<String> {
    vec![
        "storage".into(),
        "buckets".into(),
        "add-iam-policy-binding".into(),
        format!("gs://{}", bucket),
        format!("--member=serviceAccount:{}", sa),
        "--role=roles/storage.objectViewer".into(),
    ]
}

fn apply_lifecycle_and_grant(tracker: &mut Tracker, config: &Config, connection_id: &str) -> Result<()> {
    let bucket = tracker.resources().bucket_name.clone();
    if bucket.is_empty() {
        return Err(anyhow!("bucket not yet created; cold tier must run after GCS step"));
    }

    let lifecycle_file = NamedTempFile::new()?;
    std::fs::write(lifecycle_file.path(), LIFECYCLE_JSON)?;
    let bucket_uri = format!("gs://{}", bucket);
    let lifecycle_path_str = lifecycle_file.path().display().to_string();
    let out = Command::new("gsutil")
        .args(["lifecycle", "set", &lifecycle_path_str, &bucket_uri])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "gsutil lifecycle set failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let sa = connection_service_account(connection_id, &config.project, &config.region)?;
    let args = build_grant_object_viewer_args(&bucket, &sa);
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out2 = Command::new("gcloud").args(&argrefs).output()?;
    if !out2.status.success() {
        return Err(anyhow!(
            "gcloud add-iam-policy-binding failed: {}",
            String::from_utf8_lossy(&out2.stderr)
        ));
    }
    Ok(())
}
pub(crate) fn build_sentinel_export_sql(
    project: &str,
    ds: &str,
    hot: &str,
    bucket: &str,
    prefix: &str,
) -> String {
    let uri = format!("gs://{}/{}/dt=1970-01-01/sentinel-*.parquet", bucket, prefix);
    format!(
        "EXPORT DATA OPTIONS(\
            uri='{uri}', format='PARQUET', compression='ZSTD', overwrite=true) AS \
         SELECT * FROM `{project}.{ds}.{hot}` WHERE FALSE;"
    )
}

fn seed_sentinel_parquet(tracker: &mut Tracker, config: &Config) -> Result<()> {
    let bucket = tracker.resources().bucket_name.clone();
    let ds = tracker.resources().biq_query.dataset_id.clone();
    let hot = tracker.resources().biq_query.table_id.clone();
    let sql = build_sentinel_export_sql(&config.project, &ds, &hot, &bucket, PARQUET_PREFIX);
    let out = Command::new("bq")
        .args(["query", "--use_legacy_sql=false", "--project_id", &config.project, &sql])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "sentinel parquet seed failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    info!(
        "sentinel parquet written to gs://{}/{}/dt=1970-01-01/",
        bucket, PARQUET_PREFIX
    );
    Ok(())
}

pub(crate) fn build_external_table_definition_json(
    connection_qualified: &str,
    bucket: &str,
    prefix: &str,
) -> String {
    format!(
        r#"{{
  "sourceFormat": "PARQUET",
  "sourceUris": ["gs://{bucket}/{prefix}/*"],
  "connectionId": "{connection_qualified}",
  "hivePartitioningOptions": {{
    "mode": "AUTO",
    "sourceUriPrefix": "gs://{bucket}/{prefix}/",
    "requirePartitionFilter": true
  }},
  "maxStaleness": "INTERVAL 1 HOUR",
  "metadataCacheMode": "AUTOMATIC"
}}"#
    )
}

fn create_cold_table(tracker: &mut Tracker, config: &Config, conn: &str) -> Result<String> {
    let bucket = tracker.resources().bucket_name.clone();
    let dataset = tracker.resources().biq_query.dataset_id.clone();
    let table = "events_cold".to_string();
    let qualified_conn = format!("{}.{}.{}", config.project, config.region, conn);

    let def_json = build_external_table_definition_json(&qualified_conn, &bucket, PARQUET_PREFIX);
    let tmp = NamedTempFile::new()?;
    std::fs::write(tmp.path(), def_json)?;

    let fq = format!("{}:{}.{}", config.project, dataset, table);
    let def_arg = format!("--external_table_definition=@{}", tmp.path().display());
    let out = Command::new("bq")
        .args(["mk", "--table", &def_arg, &fq])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq mk cold table failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(table)
}
pub(crate) fn build_events_view_sql(project: &str, ds: &str, hot: &str, cold: &str) -> String {
    format!(
        "SELECT data, DATE(_PARTITIONTIME) AS partition_date \
         FROM `{project}.{ds}.{hot}` \
         UNION ALL \
         SELECT data, PARSE_DATE('%Y-%m-%d', dt) AS partition_date \
         FROM `{project}.{ds}.{cold}`"
    )
}

fn create_events_view(tracker: &mut Tracker, config: &Config) -> Result<String> {
    let ds = tracker.resources().biq_query.dataset_id.clone();
    let hot = tracker.resources().biq_query.table_id.clone();
    let cold = tracker.resources().cold_table_id.clone();
    let view = "events_all".to_string();
    let sql = build_events_view_sql(&config.project, &ds, &hot, &cold);
    let fq = format!("{}:{}.{}", config.project, ds, view);
    let out = Command::new("bq")
        .args(["mk", "--use_legacy_sql=false", "--view", &sql, &fq])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq mk view failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(view)
}
pub(crate) fn build_export_sql(
    project: &str,
    ds: &str,
    hot: &str,
    bucket: &str,
    prefix: &str,
    age_days: u32,
) -> String {
    let date_expr = format!("DATE_SUB(CURRENT_DATE(), INTERVAL {} DAY)", age_days);
    // The URI must be a literal at EXPORT DATA evaluation time, so we wrap the
    // whole thing in EXECUTE IMMEDIATE FORMAT and interpolate the partition
    // date into the URI and the WHERE clauses.
    format!(
        "EXECUTE IMMEDIATE FORMAT(\"\"\"\
EXPORT DATA OPTIONS(\
uri='gs://{bucket}/{prefix}/dt=%s/data-*.parquet', \
format='PARQUET', compression='ZSTD', overwrite=true) AS \
SELECT * FROM `{project}.{ds}.{hot}` WHERE _PARTITIONDATE = DATE '%s'; \
DELETE FROM `{project}.{ds}.{hot}` WHERE _PARTITIONDATE = DATE '%s';\
\"\"\", \
FORMAT_DATE('%Y-%m-%d', {date_expr}), \
FORMAT_DATE('%Y-%m-%d', {date_expr}), \
FORMAT_DATE('%Y-%m-%d', {date_expr}));"
    )
}

pub(crate) fn build_export_transfer_args(
    project: &str,
    dataset: &str,
    schedule: &str,
    params_json: &str,
) -> Vec<String> {
    vec![
        "mk".into(),
        "--transfer_config".into(),
        format!("--project_id={}", project),
        format!("--target_dataset={}", dataset),
        "--display_name=beaver_events_export".into(),
        "--data_source=scheduled_query".into(),
        format!("--schedule={}", schedule),
        format!("--params={}", params_json),
    ]
}

fn create_export_scheduled_query(tracker: &mut Tracker, config: &Config) -> Result<String> {
    let bucket = tracker.resources().bucket_name.clone();
    let ds = tracker.resources().biq_query.dataset_id.clone();
    let hot = tracker.resources().biq_query.table_id.clone();
    let sql = build_export_sql(&config.project, &ds, &hot, &bucket, PARQUET_PREFIX, EXPORT_AGE_DAYS);
    let params = serde_json::json!({ "query": sql }).to_string();

    let args = build_export_transfer_args(&config.project, &ds, EXPORT_SCHEDULE, &params);
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = Command::new("bq").args(&argrefs).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq mk transfer_config failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    // Stdout: "... 'projects/.../transferConfigs/<id>' successfully created."
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let id = stdout
        .split('\'')
        .find(|s| s.contains("transferConfigs/"))
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return Err(anyhow!("could not parse transfer config id from: {}", stdout));
    }
    Ok(id)
}

#[cfg(test)]
mod arg_tests {
    use super::*;

    #[test]
    fn export_sql_targets_partition_age_and_deletes_after() {
        let sql = build_export_sql("myproj", "myds", "events", "bucket-abc", "parquet", 13);
        let lower = sql.to_lowercase();
        assert!(lower.contains("export data"));
        assert!(lower.contains("format='parquet'"));
        assert!(lower.contains("compression='zstd'"));
        assert!(sql.contains("gs://bucket-abc/parquet/dt="));
        assert!(lower.contains("date_sub(current_date(), interval 13 day)"));
        assert!(lower.contains("delete from `myproj.myds.events`"));
        assert!(sql.contains("_PARTITIONDATE"));
    }

    #[test]
    fn export_transfer_args_have_schedule_and_params() {
        let args = build_export_transfer_args(
            "myproj",
            "myds",
            "every 24 hours",
            r#"{"query":"SELECT 1"}"#,
        );
        let joined = args.join(" ");
        assert!(joined.contains("mk"));
        assert!(joined.contains("--transfer_config"));
        assert!(joined.contains("--data_source=scheduled_query"));
        assert!(joined.contains("--target_dataset=myds"));
        assert!(joined.contains("--schedule=every 24 hours"));
        assert!(joined.contains("--params="));
    }

    #[test]
    fn events_view_sql_unions_with_partition_date_column() {
        let sql = build_events_view_sql("myproj", "myds", "events", "events_cold");
        let lower = sql.to_lowercase();
        assert!(lower.contains("union all"));
        assert!(sql.contains("`myproj.myds.events`"));
        assert!(sql.contains("`myproj.myds.events_cold`"));
        assert!(sql.contains("data"));
        assert!(sql.contains("partition_date"));
    }

    #[test]
    fn sentinel_sql_writes_zero_rows_partition_zero() {
        let sql = build_sentinel_export_sql("myproj", "myds", "events", "bucket-abc", "parquet");
        let lower = sql.to_lowercase();
        assert!(lower.contains("export data"));
        assert!(lower.contains("format='parquet'"));
        assert!(lower.contains("compression='zstd'"));
        assert!(sql.contains("gs://bucket-abc/parquet/dt=1970-01-01/"));
        assert!(lower.contains("where false"));
        assert!(sql.contains("`myproj.myds.events`"));
    }

    #[test]
    fn cold_table_definition_uses_parquet_hive_with_connection() {
        let def = build_external_table_definition_json(
            "myproj.us-east1.conn_abc",
            "bucket-abc",
            "parquet",
        );
        assert!(def.contains("\"sourceFormat\": \"PARQUET\""));
        assert!(def.contains("\"hivePartitioningOptions\""));
        assert!(def.contains("\"sourceUriPrefix\": \"gs://bucket-abc/parquet/\""));
        assert!(def.contains("\"connectionId\": \"myproj.us-east1.conn_abc\""));
        assert!(def.contains("\"mode\": \"AUTO\""));
    }

    #[test]
    fn iam_grant_args_target_bucket() {
        let args = build_grant_object_viewer_args(
            "bucket-abc",
            "biglake-sa@example.iam.gserviceaccount.com",
        );
        let joined = args.join(" ");
        assert!(joined.contains("storage"));
        assert!(joined.contains("buckets"));
        assert!(joined.contains("add-iam-policy-binding"));
        assert!(joined.contains("gs://bucket-abc"));
        assert!(joined.contains("--member=serviceAccount:biglake-sa@example.iam.gserviceaccount.com"));
        assert!(joined.contains("--role=roles/storage.objectViewer"));
    }

    #[test]
    fn biglake_connection_args_have_cloud_resource_type() {
        let args = build_biglake_connection_args("conn_abc", "myproj", "us-east1");
        let joined = args.join(" ");
        assert!(joined.contains("mk"));
        assert!(joined.contains("--connection"));
        assert!(joined.contains("--connection_type=CLOUD_RESOURCE"));
        assert!(joined.contains("--project_id=myproj"));
        assert!(joined.contains("--location=us-east1"));
        assert!(joined.contains("conn_abc"));
    }
}

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
use crate::lib::service_accounts;
use crate::lib::utilities::random_tag;

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

    // The scheduled query needs a runner identity. Create a dedicated SA,
    // grant it the BQ + GCS roles it needs, then pass it via
    // --service_account_name to bq mk --transfer_config.
    let export_sa = create_export_service_account(tracker, config)?;
    tracker.record_export_sa(export_sa.clone(), true)?;

    let sq = create_export_scheduled_query(tracker, config, &export_sa)?;
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
            "--format=json",
            id,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq show --connection failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| anyhow!("bq show --connection JSON parse failed: {} (stdout: {})", e, stdout))?;
    let sa = v.get("cloudResource")
        .and_then(|c| c.get("serviceAccountId"))
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    if sa.is_empty() {
        return Err(anyhow!("connection {} has no service account in JSON: {}", id, stdout));
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

    // BigLake SAs are created lazily and take ~30–60s to propagate to IAM after
    // `bq mk --connection`. Retry on the "Service account ... does not exist"
    // error with linear backoff.
    let mut delay = std::time::Duration::from_secs(10);
    let mut last_err = String::new();
    for attempt in 1..=8 {
        let out2 = Command::new("gcloud").args(&argrefs).output()?;
        if out2.status.success() {
            return Ok(());
        }
        last_err = String::from_utf8_lossy(&out2.stderr).to_string();
        if !last_err.contains("does not exist") {
            return Err(anyhow!("gcloud add-iam-policy-binding failed: {}", last_err));
        }
        info!("biglake SA not yet visible to IAM (attempt {}/8); waiting {:?}", attempt, delay);
        std::thread::sleep(delay);
        delay = std::cmp::min(delay + std::time::Duration::from_secs(10), std::time::Duration::from_secs(30));
    }
    Err(anyhow!(
        "gcloud add-iam-policy-binding failed after retries: {}",
        last_err
    ))
}
pub(crate) fn build_sentinel_export_sql(
    project: &str,
    ds: &str,
    hot: &str,
    bucket: &str,
    prefix: &str,
) -> String {
    // BQ can't export JSON-typed columns to Parquet, so we cast to STRING on
    // the way out. The view re-parses with SAFE.PARSE_JSON on the cold side.
    let uri = format!("gs://{}/{}/dt=1970-01-01/sentinel-*.parquet", bucket, prefix);
    format!(
        "EXPORT DATA OPTIONS(\
            uri='{uri}', format='PARQUET', compression='ZSTD', overwrite=true) AS \
         SELECT TO_JSON_STRING(data) AS data FROM `{project}.{ds}.{hot}` WHERE FALSE;"
    )
}

fn seed_sentinel_parquet(tracker: &mut Tracker, config: &Config) -> Result<()> {
    let bucket = tracker.resources().bucket_name.clone();
    let ds = tracker.resources().biq_query.dataset_id.clone();
    let hot = tracker.resources().biq_query.table_id.clone();
    let sql = build_sentinel_export_sql(&config.project, &ds, &hot, &bucket, PARQUET_PREFIX);
    let out = Command::new("bq")
        .args([
            "query", "--use_legacy_sql=false",
            "--project_id", &config.project,
            "--location", &config.region,
            &sql,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "sentinel parquet seed failed (exit {}): stderr={} stdout={}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout),
        ));
    }
    info!(
        "sentinel parquet written to gs://{}/{}/dt=1970-01-01/",
        bucket, PARQUET_PREFIX
    );
    Ok(())
}

pub(crate) fn build_cold_table_ddl(
    project: &str,
    dataset: &str,
    table: &str,
    connection_qualified: &str,
    bucket: &str,
    prefix: &str,
) -> String {
    format!(
        "CREATE EXTERNAL TABLE `{project}.{dataset}.{table}` \
         WITH PARTITION COLUMNS \
         WITH CONNECTION `{connection_qualified}` \
         OPTIONS ( \
           format = 'PARQUET', \
           uris = ['gs://{bucket}/{prefix}/*'], \
           hive_partition_uri_prefix = 'gs://{bucket}/{prefix}/', \
           require_hive_partition_filter = true, \
           max_staleness = INTERVAL '1' HOUR, \
           metadata_cache_mode = 'AUTOMATIC' \
         );"
    )
}

fn create_cold_table(tracker: &mut Tracker, config: &Config, conn: &str) -> Result<String> {
    let bucket = tracker.resources().bucket_name.clone();
    let dataset = tracker.resources().biq_query.dataset_id.clone();
    let table = "events_cold".to_string();
    let qualified_conn = format!("{}.{}.{}", config.project, config.region, conn);

    let ddl = build_cold_table_ddl(
        &config.project, &dataset, &table, &qualified_conn, &bucket, PARQUET_PREFIX,
    );
    let out = Command::new("bq")
        .args([
            "query", "--use_legacy_sql=false",
            "--project_id", &config.project,
            "--location", &config.region,
            &ddl,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "create cold table DDL failed (exit {}): stderr={} stdout={}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout),
        ));
    }
    Ok(table)
}
pub(crate) fn build_events_view_sql(project: &str, ds: &str, hot: &str, cold: &str) -> String {
    // Cold rows store `data` as STRING (JSON cannot be Parquet-exported), so
    // re-parse it here for analyst ergonomics. Hot stays native JSON.
    // Hive AUTO mode infers `dt` as DATE from the `dt=YYYY-MM-DD` folder name,
    // so no PARSE_DATE is needed on the cold side.
    format!(
        "SELECT data, DATE(_PARTITIONTIME) AS partition_date \
         FROM `{project}.{ds}.{hot}` \
         UNION ALL \
         SELECT SAFE.PARSE_JSON(data) AS data, dt AS partition_date \
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
        .args([
            "mk", "--use_legacy_sql=false",
            "--location", &config.region,
            "--view", &sql, &fq,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq mk view failed (exit {}): stderr={} stdout={}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout),
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
SELECT TO_JSON_STRING(data) AS data FROM `{project}.{ds}.{hot}` WHERE _PARTITIONDATE = DATE '%s'; \
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
    region: &str,
    schedule: &str,
    params_json: &str,
) -> Vec<String> {
    vec![
        "mk".into(),
        "--transfer_config".into(),
        format!("--project_id={}", project),
        format!("--target_dataset={}", dataset),
        format!("--location={}", region),
        "--display_name=beaver_events_export".into(),
        "--data_source=scheduled_query".into(),
        format!("--schedule={}", schedule),
        format!("--params={}", params_json),
    ]
}

fn create_export_service_account(tracker: &mut Tracker, config: &Config) -> Result<String> {
    let bucket = tracker.resources().bucket_name.clone();
    let account_id = format!("beaver-export-{}", random_tag(6));
    let sa = service_accounts::create_sa(&account_id, "Beaver export job", config)?;
    let email = sa.email;

    // jobUser + dataEditor at project, objectAdmin on the parquet bucket.
    service_accounts::grant_project(&config.project, &email, "roles/bigquery.jobUser")?;
    service_accounts::grant_project(&config.project, &email, "roles/bigquery.dataEditor")?;
    service_accounts::grant_bucket(&bucket, &email, "roles/storage.objectAdmin")?;

    // The user invoking `bq mk --transfer_config --service_account_name=<email>`
    // must be able to impersonate the SA. Without this, BQ DTS refuses to
    // create the transfer with a PERMISSION_DENIED on serviceAccountTokenCreator.
    let invoker = current_user_principal()?;
    grant_sa_token_creator(&email, &invoker, &config.project)?;

    Ok(email)
}

fn current_user_principal() -> Result<String> {
    let out = Command::new("gcloud")
        .args(["config", "get-value", "account"])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("gcloud config get-value account failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() {
        return Err(anyhow!("no active gcloud account configured"));
    }
    // Service accounts vs user accounts use different IAM principal prefixes.
    if raw.ends_with(".iam.gserviceaccount.com") {
        Ok(format!("serviceAccount:{}", raw))
    } else {
        Ok(format!("user:{}", raw))
    }
}

fn grant_sa_token_creator(sa_email: &str, member: &str, project: &str) -> Result<()> {
    let out = Command::new("gcloud")
        .args([
            "iam", "service-accounts", "add-iam-policy-binding", sa_email,
            "--member", member,
            "--role", "roles/iam.serviceAccountTokenCreator",
            "--project", project,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "grant tokenCreator on {} to {} failed: {}",
            sa_email, member, String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn create_export_scheduled_query(tracker: &mut Tracker, config: &Config, export_sa: &str) -> Result<String> {
    let bucket = tracker.resources().bucket_name.clone();
    let ds = tracker.resources().biq_query.dataset_id.clone();
    let hot = tracker.resources().biq_query.table_id.clone();
    let sql = build_export_sql(&config.project, &ds, &hot, &bucket, PARQUET_PREFIX, EXPORT_AGE_DAYS);
    let params = serde_json::json!({ "query": sql }).to_string();

    let args = build_export_transfer_args(&config.project, &ds, &config.region, EXPORT_SCHEDULE, &params);
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let sa_flag = format!("--service_account_name={}", export_sa);
    let out = Command::new("bq").args(&argrefs).arg(&sa_flag).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "bq mk transfer_config failed (exit {}): stderr={} stdout={}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout),
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
mod integration_tests {
    use super::*;
    use crate::lib::bq;
    use crate::lib::resources::Tracker;
    use crate::lib::test_helpers::{test_config, tempdir_resources, bq_dataset_exists};

    /// End-to-end smoke against a real GCP project. Provisions a hot dataset+table,
    /// runs cold_storage::create, then tears everything down. Requires:
    ///   - gcloud + bq authenticated against BEAVER_TEST_PROJECT (default in test_helpers).
    ///   - A pre-existing GCS bucket whose name is passed via BEAVER_TEST_BUCKET — we
    ///     do NOT provision one here because gcs::create_bucket has its own ignored
    ///     tests and we want this smoke focused on cold-tier wiring.
    #[test]
    #[ignore]
    fn create_cold_tier_against_real_gcp() {
        let config = test_config();
        let bucket = std::env::var("BEAVER_TEST_BUCKET")
            .expect("set BEAVER_TEST_BUCKET to a real GCS bucket in the test project");

        let (_dir, mut res) = tempdir_resources();
        res.bucket_name = bucket;

        // Provision hot dataset + table via the real bq::create path so the cold
        // tier has something to point at.
        {
            let mut tracker = Tracker::new(&mut res);
            bq::create(&mut tracker, &config).expect("bq::create failed");
        }
        let dataset_id = res.biq_query.dataset_id.clone();
        assert!(bq_dataset_exists(&dataset_id, &config.project));

        // Now exercise cold_storage::create end-to-end.
        let result = {
            let mut tracker = Tracker::new(&mut res);
            create(&mut tracker, &config)
        };

        // Teardown FIRST, regardless of outcome — we never want to leak state.
        let connection_id = res.biglake_connection_id.clone();
        let cold_table = res.cold_table_id.clone();
        let view = res.events_view_id.clone();
        let sq = res.export_scheduled_query_id.clone();
        let export_sa = res.export_sa_email.clone();

        if !sq.is_empty() {
            let _ = destroy_scheduled_query(&sq, &config.project);
        }
        if !export_sa.is_empty() {
            let _ = crate::lib::service_accounts::delete_sa(&export_sa, &config.project);
        }
        if !view.is_empty() {
            let _ = destroy_view(&dataset_id, &view, &config.project);
        }
        if !cold_table.is_empty() {
            let _ = destroy_cold_table(&dataset_id, &cold_table, &config.project);
        }
        if !connection_id.is_empty() {
            let _ = destroy_connection(&connection_id, &config.project, &config.region);
        }
        let _ = bq::delete_dataset(&dataset_id, &config.project);

        // Now assert outcome.
        result.expect("cold_storage::create failed");
        assert!(!connection_id.is_empty(), "biglake connection not recorded");
        assert!(!cold_table.is_empty(), "cold table not recorded");
        assert!(!view.is_empty(), "events view not recorded");
        assert!(!sq.is_empty(), "export scheduled query not recorded");
    }
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
            "us-east1",
            "every 24 hours",
            r#"{"query":"SELECT 1"}"#,
        );
        let joined = args.join(" ");
        assert!(joined.contains("mk"));
        assert!(joined.contains("--transfer_config"));
        assert!(joined.contains("--data_source=scheduled_query"));
        assert!(joined.contains("--target_dataset=myds"));
        assert!(joined.contains("--location=us-east1"));
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
    fn cold_table_ddl_uses_parquet_hive_with_connection() {
        let ddl = build_cold_table_ddl(
            "myproj", "myds", "events_cold",
            "myproj.us-east1.conn_abc",
            "bucket-abc",
            "parquet",
        );
        assert!(ddl.contains("CREATE EXTERNAL TABLE"));
        assert!(ddl.contains("`myproj.myds.events_cold`"));
        assert!(ddl.contains("WITH CONNECTION `myproj.us-east1.conn_abc`"));
        assert!(ddl.contains("format = 'PARQUET'"));
        assert!(ddl.contains("uris = ['gs://bucket-abc/parquet/*']"));
        assert!(ddl.contains("hive_partition_uri_prefix = 'gs://bucket-abc/parquet/'"));
        assert!(ddl.contains("require_hive_partition_filter = true"));
        assert!(ddl.contains("WITH PARTITION COLUMNS"));
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

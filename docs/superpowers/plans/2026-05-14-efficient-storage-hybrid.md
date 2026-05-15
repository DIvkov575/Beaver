# Efficient Storage (Tiering Only) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (chosen) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a cold-storage tier underneath the existing BigQuery `data: JSON` events table — daily-partitioned hot table with 14-day retention, automatic rollout of older partitions to zstd-Parquet on GCS, exposed back through a BigLake external table and a unified view — without changing the producer JSON shape or the existing `data: JSON` column.

**Architecture:** The single existing BigQuery table gains DAY ingestion-time partitioning and a 14-day partition expiration. A BigQuery scheduled query runs daily, exports the partition that is 13 days old to `gs://<bucket>/parquet/dt=YYYY-MM-DD/*.parquet` (Parquet, ZSTD, Hive-partitioned), then deletes that partition from the hot table. A BigLake external table `events_cold` reads the parquet prefix; a view `events_all` unions hot + cold so analysts query one surface. GCS lifecycle moves parquet from Standard → Nearline (30d) → Coldline (90d) → Archive (365d). **The Pub/Sub BQ subscription, producer code, and Dataflow detections job are untouched.**

**Tech Stack:** Rust (binary `beaver`), `bq` CLI, `gcloud` CLI, `gsutil` CLI, BigQuery scheduled queries (Data Transfer Service), BigLake connections, GCS.

**Knobs (constants at top of `cold_storage.rs` — defaults shown):**
- `HOT_RETENTION_DAYS = 14` (hot-table partition expiration)
- `EXPORT_AGE_DAYS = 13` (export partitions this old; must be `<` HOT_RETENTION_DAYS to avoid the export racing the expiry)
- `EXPORT_SCHEDULE = "every 24 hours"`
- `PARQUET_PREFIX = "parquet"` (under the existing beaver bucket)
- GCS lifecycle: Nearline @ 30d, Coldline @ 90d, Archive @ 365d

**Non-goals (defer to a future plan):**
- Typed/normalized columns (`source`, `event_type`, `host`, `user`, …). Adding these requires producer changes (Vector VRL remap or a transformer) which we are not doing here.
- Clustering. (You can only cluster on real columns; the JSON column doesn't help here.)

---

## File Structure

**New files:**
- `src/lib/cold_storage.rs` — BigLake connection, external table, view, scheduled query, lifecycle, sentinel-parquet seeding.
- `src/lib/schemas/lifecycle.json` — embedded GCS lifecycle policy (read at compile time via `include_str!`).

**Modified files:**
- `src/lib/bq.rs:100-116` — `create_table` adds DAY partitioning + 14-day expiration on a hidden ingestion-time column. **Column stays `data: JSON`.**
- `src/lib/resources.rs:10-36` — extend `Resources` with cold-tier fields; add `record_*`/`forget_*` methods.
- `src/lib/mod.rs` — `pub mod cold_storage;`.
- `src/commands/deploy.rs:55-57` — new `step("Cold tier", …)` after BQ + GCS bucket are both ready.
- `src/commands/destroy.rs:10-90` — new `DeleteStep` variants + plan ordering + execute branches.
- `src/lib/dashboard.rs` — tiles for parquet bytes + scheduled-query success.
- `Cargo.toml` — add `serde_json` (used for transfer-config params JSON).

---

## Task 1: Partition + age-out the hot table

**Files:**
- Modify: `src/lib/bq.rs:100-116` (`create_table`).

- [ ] **Step 1: Write failing unit test**

Add to `src/lib/bq.rs` above `mod integration_tests` (around line 146):

```rust
#[cfg(test)]
mod arg_tests {
    use super::*;

    #[test]
    fn create_table_args_include_ingestion_partition_and_expiration() {
        let args = build_create_table_args("myproj:myds.t1", 14 * 24 * 60 * 60);
        let joined = args.join(" ");
        assert!(joined.contains("--time_partitioning_type=DAY"));
        assert!(joined.contains("--time_partitioning_expiration=1209600"));
        assert!(joined.contains("--require_partition_filter=true"));
        assert!(joined.contains("data:JSON"));
        assert!(joined.contains("myproj:myds.t1"));
    }
}
```

- [ ] **Step 2: Run, expect FAIL**

```bash
cargo test --lib bq::arg_tests
```
Expected: FAIL — `build_create_table_args` not defined.

- [ ] **Step 3: Implement**

Replace `create_table` in `src/lib/bq.rs:100-116` with:

```rust
pub(crate) fn build_create_table_args(fq_table: &str, partition_expiration_secs: u64) -> Vec<String> {
    vec![
        "mk".to_string(),
        "--table".to_string(),
        "--time_partitioning_type=DAY".to_string(),
        format!("--time_partitioning_expiration={}", partition_expiration_secs),
        "--require_partition_filter=true".to_string(),
        fq_table.to_string(),
        "data:JSON".to_string(),
    ]
}

pub fn create_table(dataset_id: &str, table_id: &str, project_id: &str) -> Result<()> {
    let fq = format!("{}:{}.{}", project_id, dataset_id, table_id);
    let args = build_create_table_args(&fq, 14 * 24 * 60 * 60);
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = Command::new("bq").args(&argrefs).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("bq create_table failed: {}", stderr));
    }
    info!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
```

`--time_partitioning_type=DAY` without `--time_partitioning_field` partitions on **ingestion time** (`_PARTITIONTIME` / `_PARTITIONDATE` pseudo-columns), which is exactly what we want without touching the schema.

- [ ] **Step 4: Run, expect PASS**

```bash
cargo test --lib bq::arg_tests
cargo build
```
Expected: 1 passed; build green.

- [ ] **Step 5: Commit**

```bash
git add src/lib/bq.rs
git commit -m "feat(bq): day-partition hot events table, 14d expiration"
```

---

## Task 2: Extend `Resources` with cold-tier fields

**Files:**
- Modify: `src/lib/resources.rs:10-36` (struct), `:38-62` (`empty`), tracker methods after line 100.

- [ ] **Step 1: Read `src/lib/config.rs` and confirm how to construct a `Config` in tests**

```bash
grep -n "pub struct Config\|impl Config\|Default" src/lib/config.rs | head
```
Inspect output. If `Config` doesn't `derive(Default)`, the test below must build one with explicit fields. Adjust the test as needed before running.

- [ ] **Step 2: Write failing test**

Append to `src/lib/resources.rs`:

```rust
#[cfg(test)]
mod cold_field_tests {
    use super::*;
    use crate::lib::config::Config;
    use std::path::PathBuf;

    fn cfg() -> Config {
        // If Config has no Default, replace with explicit fields visible in config.rs.
        Config { project: "p".into(), region: "r".into(), ..Default::default() }
    }

    #[test]
    fn cold_tier_fields_roundtrip() {
        let mut r = Resources::empty(&cfg(), &PathBuf::from("."));
        r.biglake_connection_id = "conn1".into();
        r.cold_bucket_prefix = "parquet".into();
        r.cold_table_id = "events_cold".into();
        r.events_view_id = "events_all".into();
        r.export_scheduled_query_id = "sq1".into();

        let yaml = serde_yaml::to_string(&r).unwrap();
        let back: Resources = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.biglake_connection_id, "conn1");
        assert_eq!(back.cold_bucket_prefix, "parquet");
        assert_eq!(back.cold_table_id, "events_cold");
        assert_eq!(back.events_view_id, "events_all");
        assert_eq!(back.export_scheduled_query_id, "sq1");
    }
}
```

- [ ] **Step 3: Run, expect FAIL**

```bash
cargo test --lib resources::cold_field_tests
```

- [ ] **Step 4: Add fields and tracker methods**

In `Resources` struct (`resources.rs:10-36`), after `log_metric_name`:

```rust
    #[serde(default)] pub biglake_connection_id: String,
    #[serde(default)] pub cold_bucket_prefix: String,
    #[serde(default)] pub cold_table_id: String,
    #[serde(default)] pub events_view_id: String,
    #[serde(default)] pub export_scheduled_query_id: String,
```

In `Resources::empty`, initialize each to `String::new()`.

Add tracker methods in `impl<'a> Tracker<'a>` after `forget_log_metric` (or last `forget_*`):

```rust
    pub fn record_biglake_connection(&mut self, id: String) -> Result<()> { self.res.biglake_connection_id = id; self.persist() }
    pub fn forget_biglake_connection(&mut self) -> Result<()> { self.res.biglake_connection_id.clear(); self.persist() }
    pub fn record_cold_bucket_prefix(&mut self, p: String) -> Result<()> { self.res.cold_bucket_prefix = p; self.persist() }
    pub fn forget_cold_bucket_prefix(&mut self) -> Result<()> { self.res.cold_bucket_prefix.clear(); self.persist() }
    pub fn record_cold_table(&mut self, id: String) -> Result<()> { self.res.cold_table_id = id; self.persist() }
    pub fn forget_cold_table(&mut self) -> Result<()> { self.res.cold_table_id.clear(); self.persist() }
    pub fn record_events_view(&mut self, id: String) -> Result<()> { self.res.events_view_id = id; self.persist() }
    pub fn forget_events_view(&mut self) -> Result<()> { self.res.events_view_id.clear(); self.persist() }
    pub fn record_export_scheduled_query(&mut self, id: String) -> Result<()> { self.res.export_scheduled_query_id = id; self.persist() }
    pub fn forget_export_scheduled_query(&mut self) -> Result<()> { self.res.export_scheduled_query_id.clear(); self.persist() }
```

- [ ] **Step 5: Run, expect PASS**

```bash
cargo test --lib resources::cold_field_tests
```

- [ ] **Step 6: Commit**

```bash
git add src/lib/resources.rs
git commit -m "feat(resources): track cold-tier BigLake/connection/view/scheduled-query IDs"
```

---

## Task 3: `cold_storage` module skeleton + embedded lifecycle JSON

**Files:**
- Create: `src/lib/cold_storage.rs`
- Create: `src/lib/schemas/lifecycle.json`
- Modify: `src/lib/mod.rs`
- Modify: `Cargo.toml` — add `serde_json = "1.0"`.

- [ ] **Step 1: Write lifecycle JSON**

Path: `src/lib/schemas/lifecycle.json`

```json
{
  "lifecycle": {
    "rule": [
      {"action": {"type": "SetStorageClass", "storageClass": "NEARLINE"},
       "condition": {"age": 30, "matchesPrefix": ["parquet/"]}},
      {"action": {"type": "SetStorageClass", "storageClass": "COLDLINE"},
       "condition": {"age": 90, "matchesPrefix": ["parquet/"]}},
      {"action": {"type": "SetStorageClass", "storageClass": "ARCHIVE"},
       "condition": {"age": 365, "matchesPrefix": ["parquet/"]}}
    ]
  }
}
```

- [ ] **Step 2: Add serde_json to Cargo.toml**

In `[dependencies]` block of `Cargo.toml`, after `serde_yaml`:

```toml
serde_json = "1.0"
```

- [ ] **Step 3: Create skeleton**

Path: `src/lib/cold_storage.rs`

```rust
//! Cold tier: parquet on GCS exposed via a BigLake external table, plus a
//! scheduled query that rolls partitions out of the hot BQ table daily.

use std::process::Command;
use anyhow::{anyhow, Result};
use log::{error, info};
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
        return Err(anyhow!("bq rm transfer_config failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

pub fn destroy_view(dataset: &str, view: &str, project: &str) -> Result<()> {
    let fq = format!("{}:{}.{}", project, dataset, view);
    let out = Command::new("bq").args(["rm", "-f", "-t", &fq]).output()?;
    if !out.status.success() {
        return Err(anyhow!("bq rm view failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

pub fn destroy_cold_table(dataset: &str, table: &str, project: &str) -> Result<()> {
    let fq = format!("{}:{}.{}", project, dataset, table);
    let out = Command::new("bq").args(["rm", "-f", "-t", &fq]).output()?;
    if !out.status.success() {
        return Err(anyhow!("bq rm cold table failed: {}",
            String::from_utf8_lossy(&out.stderr)));
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
        return Err(anyhow!("bq rm connection failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

// Stubs filled in by Tasks 4–8.
fn create_biglake_connection(_config: &Config) -> Result<String> { Err(anyhow!("Task 4")) }
fn apply_lifecycle_and_grant(_t: &mut Tracker, _c: &Config, _conn: &str) -> Result<()> { Err(anyhow!("Task 5")) }
fn seed_sentinel_parquet(_t: &mut Tracker, _c: &Config) -> Result<()> { Err(anyhow!("Task 6")) }
fn create_cold_table(_t: &mut Tracker, _c: &Config, _conn: &str) -> Result<String> { Err(anyhow!("Task 6")) }
fn create_events_view(_t: &mut Tracker, _c: &Config) -> Result<String> { Err(anyhow!("Task 7")) }
fn create_export_scheduled_query(_t: &mut Tracker, _c: &Config) -> Result<String> { Err(anyhow!("Task 8")) }
```

- [ ] **Step 4: Wire into `mod.rs`**

In `src/lib/mod.rs`, add `pub mod cold_storage;` alongside the other module declarations.

- [ ] **Step 5: Build**

```bash
cargo build
```
Expected: success (unused-function warnings on stubs are OK; `LIFECYCLE_JSON` is unused until Task 5).

- [ ] **Step 6: Commit**

```bash
git add src/lib/cold_storage.rs src/lib/schemas/lifecycle.json src/lib/mod.rs Cargo.toml Cargo.lock
git commit -m "feat(cold-storage): module skeleton + embedded lifecycle JSON"
```

---

## Task 4: BigLake connection

**Files:**
- Modify: `src/lib/cold_storage.rs` — implement `create_biglake_connection` + `connection_service_account` helper.

- [ ] **Step 1: Write failing test**

Append to `cold_storage.rs`:

```rust
#[cfg(test)]
mod arg_tests {
    use super::*;

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
```

- [ ] **Step 2: Run, expect FAIL.**

```bash
cargo test --lib cold_storage::arg_tests
```

- [ ] **Step 3: Implement**

Replace the `create_biglake_connection` stub with:

```rust
pub(crate) fn build_biglake_connection_args(id: &str, project: &str, region: &str) -> Vec<String> {
    vec![
        "mk".into(), "--connection".into(),
        "--connection_type=CLOUD_RESOURCE".into(),
        format!("--project_id={}", project),
        format!("--location={}", region),
        id.into(),
    ]
}

fn create_biglake_connection(config: &Config) -> Result<String> {
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric).take(6).map(char::from)
        .map(|c| c.to_ascii_lowercase()).collect();
    let id = format!("beaver_biglake_{}", suffix);
    let args = build_biglake_connection_args(&id, &config.project, &config.region);
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = Command::new("bq").args(&argrefs).output()?;
    if !out.status.success() {
        return Err(anyhow!("bq mk --connection failed: {}",
            String::from_utf8_lossy(&out.stderr)));
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
        return Err(anyhow!("bq show --connection failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    let sa = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sa.is_empty() { return Err(anyhow!("connection {} has no service account", id)); }
    Ok(sa)
}
```

- [ ] **Step 4: Run, expect PASS.**

```bash
cargo test --lib cold_storage::arg_tests
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/cold_storage.rs
git commit -m "feat(cold-storage): create BigLake CLOUD_RESOURCE connection + SA lookup"
```

---

## Task 5: Apply lifecycle JSON + grant connection SA read

**Files:**
- Modify: `src/lib/cold_storage.rs` — implement `apply_lifecycle_and_grant`.

- [ ] **Step 1: Write failing test**

Append to `cold_storage::arg_tests`:

```rust
    #[test]
    fn iam_grant_args_target_bucket() {
        let args = build_grant_object_viewer_args(
            "bucket-abc", "biglake-sa@example.iam.gserviceaccount.com");
        let joined = args.join(" ");
        assert!(joined.contains("storage"));
        assert!(joined.contains("buckets"));
        assert!(joined.contains("add-iam-policy-binding"));
        assert!(joined.contains("gs://bucket-abc"));
        assert!(joined.contains("--member=serviceAccount:biglake-sa@example.iam.gserviceaccount.com"));
        assert!(joined.contains("--role=roles/storage.objectViewer"));
    }
```

- [ ] **Step 2: Run, expect FAIL.**

- [ ] **Step 3: Implement**

Add to `cold_storage.rs`:

```rust
pub(crate) fn build_grant_object_viewer_args(bucket: &str, sa: &str) -> Vec<String> {
    vec![
        "storage".into(), "buckets".into(), "add-iam-policy-binding".into(),
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

    // 1. Write the embedded lifecycle JSON to a tempfile and apply it.
    let lifecycle_file = NamedTempFile::new()?;
    std::fs::write(lifecycle_file.path(), LIFECYCLE_JSON)?;
    let bucket_uri = format!("gs://{}", bucket);
    let lifecycle_path_str = lifecycle_file.path().display().to_string();
    let out = Command::new("gsutil")
        .args(["lifecycle", "set", &lifecycle_path_str, &bucket_uri])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("gsutil lifecycle set failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }

    // 2. Grant the BigLake connection SA read on the bucket.
    let sa = connection_service_account(connection_id, &config.project, &config.region)?;
    let args = build_grant_object_viewer_args(&bucket, &sa);
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out2 = Command::new("gcloud").args(&argrefs).output()?;
    if !out2.status.success() {
        return Err(anyhow!("gcloud add-iam-policy-binding failed: {}",
            String::from_utf8_lossy(&out2.stderr)));
    }
    Ok(())
}
```

- [ ] **Step 4: Run, expect PASS.**

- [ ] **Step 5: Commit**

```bash
git add src/lib/cold_storage.rs
git commit -m "feat(cold-storage): apply GCS lifecycle + grant BigLake SA read"
```

---

## Task 6: Sentinel parquet + cold external table

**Files:**
- Modify: `src/lib/cold_storage.rs` — `seed_sentinel_parquet` + `create_cold_table`.

The empty-prefix problem: `bq mk --table --external_table_definition=...` over `gs://.../parquet/*` fails if no parquet files exist yet. We seed a 0-row parquet first so the schema is discoverable.

- [ ] **Step 1: Write failing test for SQL & definition shape**

Append to `cold_storage::arg_tests`:

```rust
    #[test]
    fn sentinel_sql_writes_zero_rows_partition_zero() {
        let sql = build_sentinel_export_sql(
            "myproj", "myds", "events", "bucket-abc", "parquet");
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
            "myproj.us-east1.conn_abc", "bucket-abc", "parquet");
        assert!(def.contains("\"sourceFormat\": \"PARQUET\""));
        assert!(def.contains("\"hivePartitioningOptions\""));
        assert!(def.contains("\"sourceUriPrefix\": \"gs://bucket-abc/parquet/\""));
        assert!(def.contains("\"connectionId\": \"myproj.us-east1.conn_abc\""));
        assert!(def.contains("\"mode\": \"AUTO\""));
    }
```

- [ ] **Step 2: Run, expect FAIL.**

- [ ] **Step 3: Implement**

```rust
pub(crate) fn build_sentinel_export_sql(
    project: &str, ds: &str, hot: &str, bucket: &str, prefix: &str,
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
        return Err(anyhow!("sentinel parquet seed failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    info!("sentinel parquet written to gs://{}/{}/dt=1970-01-01/", bucket, PARQUET_PREFIX);
    Ok(())
}

pub(crate) fn build_external_table_definition_json(
    connection_qualified: &str, bucket: &str, prefix: &str,
) -> String {
    format!(r#"{{
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
}}"#)
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
    let out = Command::new("bq").args(["mk", "--table", &def_arg, &fq]).output()?;
    if !out.status.success() {
        return Err(anyhow!("bq mk cold table failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    Ok(table)
}
```

- [ ] **Step 4: Run, expect PASS.**

```bash
cargo test --lib cold_storage::arg_tests
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/cold_storage.rs
git commit -m "feat(cold-storage): sentinel parquet + BigLake hive-partitioned external table"
```

---

## Task 7: Unified `events_all` view

**Files:**
- Modify: `src/lib/cold_storage.rs` — `create_events_view`.

The hot table exposes `_PARTITIONTIME` as a pseudo-column; the cold (Hive-partitioned) table exposes `dt` as a STRING. The view normalizes them both to a `partition_date DATE` column so analysts can filter once.

- [ ] **Step 1: Write failing test**

Append to `cold_storage::arg_tests`:

```rust
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
```

- [ ] **Step 2: Run, expect FAIL.**

- [ ] **Step 3: Implement**

```rust
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
    let out = Command::new("bq").args(["mk", "--use_legacy_sql=false", "--view", &sql, &fq]).output()?;
    if !out.status.success() {
        return Err(anyhow!("bq mk view failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    Ok(view)
}
```

- [ ] **Step 4: Run, expect PASS.**

- [ ] **Step 5: Commit**

```bash
git add src/lib/cold_storage.rs
git commit -m "feat(cold-storage): unified events_all view exposing partition_date"
```

---

## Task 8: Daily export scheduled query

**Files:**
- Modify: `src/lib/cold_storage.rs` — `create_export_scheduled_query`.

- [ ] **Step 1: Write failing test**

Append to `cold_storage::arg_tests`:

```rust
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
            "myproj", "myds", "every 24 hours", r#"{"query":"SELECT 1"}"#);
        let joined = args.join(" ");
        assert!(joined.contains("mk"));
        assert!(joined.contains("--transfer_config"));
        assert!(joined.contains("--data_source=scheduled_query"));
        assert!(joined.contains("--target_dataset=myds"));
        assert!(joined.contains("--schedule=every 24 hours"));
        assert!(joined.contains("--params="));
    }
```

- [ ] **Step 2: Run, expect FAIL.**

- [ ] **Step 3: Implement**

```rust
pub(crate) fn build_export_sql(
    project: &str, ds: &str, hot: &str,
    bucket: &str, prefix: &str, age_days: u32,
) -> String {
    let date_expr = format!("DATE_SUB(CURRENT_DATE(), INTERVAL {} DAY)", age_days);
    // Embed the actual partition date in the uri so files land in the right
    // hive-partitioned dt= folder. The wildcard expands per-shard.
    let uri = format!(
        "gs://{}/{}/dt=' || FORMAT_DATE('%Y-%m-%d', {}) || '/data-*.parquet",
        bucket, prefix, date_expr
    );
    format!(
        "EXECUTE IMMEDIATE FORMAT(\
           \"EXPORT DATA OPTIONS(uri='{uri}', format='PARQUET', compression='ZSTD', overwrite=true) AS \
            SELECT * FROM `{project}.{ds}.{hot}` WHERE _PARTITIONDATE = %T; \
            DELETE FROM `{project}.{ds}.{hot}` WHERE _PARTITIONDATE = %T;\", \
           {date_expr}, {date_expr});"
    )
}

pub(crate) fn build_export_transfer_args(
    project: &str, dataset: &str, schedule: &str, params_json: &str,
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
        return Err(anyhow!("bq mk transfer_config failed: {}",
            String::from_utf8_lossy(&out.stderr)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    // Stdout looks like: "Transfer configuration 'projects/.../transferConfigs/abc...' successfully created."
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
```

- [ ] **Step 4: Run, expect PASS.**

```bash
cargo test --lib cold_storage::arg_tests
cargo build
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/cold_storage.rs
git commit -m "feat(cold-storage): daily EXPORT DATA → parquet + partition delete scheduled query"
```

---

## Task 9: Wire deploy + destroy

**Files:**
- Modify: `src/commands/deploy.rs` (add cold-tier step after BQ + bucket).
- Modify: `src/commands/destroy.rs` (variants, plan, execute).

- [ ] **Step 1: Add deploy step**

In `src/commands/deploy.rs`:

a) At the top of the file's `use` block (where other `use crate::lib::*` lines live), add:

```rust
use crate::lib::cold_storage;
```

b) After the **last** of (BQ create, bucket create); locate where `gcs::create_bucket` is invoked (`deploy.rs:57`). Insert a new `step` **after** the bucket step but before the vector SA step:

```rust
    step("Cold tier (BigLake + lifecycle + scheduled export)",
        || cold_storage::create(&mut tracker, &config))?;
```

c) Verify by reading 5 lines of context that the order is now: BQ → Pub/Sub → bucket → **cold tier** → vector SA. If Pub/Sub runs before bucket and you want cold tier strictly after both, place it where shown above (right after bucket).

- [ ] **Step 2: Add destroy variants**

In `src/commands/destroy.rs` `DeleteStep` enum (`destroy.rs:11-27`):

```rust
    ExportScheduledQuery(String),
    EventsView(String),
    ColdTable(String),
    BigLakeConnection(String),
```

- [ ] **Step 3: Add plan entries**

In `plan()` (`destroy.rs:34-89`), **before** the `BqDataset` block (which is around `:65-67`), insert:

```rust
    if !res.export_scheduled_query_id.is_empty() {
        steps.push(DeleteStep::ExportScheduledQuery(res.export_scheduled_query_id.clone()));
    }
    if !res.events_view_id.is_empty() {
        steps.push(DeleteStep::EventsView(res.events_view_id.clone()));
    }
    if !res.cold_table_id.is_empty() {
        steps.push(DeleteStep::ColdTable(res.cold_table_id.clone()));
    }
    if !res.biglake_connection_id.is_empty() {
        steps.push(DeleteStep::BigLakeConnection(res.biglake_connection_id.clone()));
    }
```

- [ ] **Step 4: Add execute branches**

a) In the `forget`-dispatch match block in `execute` (around `destroy.rs:102-118`), add:

```rust
    DeleteStep::ExportScheduledQuery(_) => tracker.forget_export_scheduled_query(),
    DeleteStep::EventsView(_) => tracker.forget_events_view(),
    DeleteStep::ColdTable(_) => tracker.forget_cold_table(),
    DeleteStep::BigLakeConnection(_) => tracker.forget_biglake_connection(),
```

b) Find the closure passed to `execute` (the one that actually calls `bq::delete_dataset`, `gcs::delete_bucket`, etc. — search for `BqDataset(` to find it). Add branches:

```rust
    DeleteStep::ExportScheduledQuery(id) => cold_storage::destroy_scheduled_query(id, &config.project),
    DeleteStep::EventsView(view) => cold_storage::destroy_view(&res.biq_query.dataset_id, view, &config.project),
    DeleteStep::ColdTable(t) => cold_storage::destroy_cold_table(&res.biq_query.dataset_id, t, &config.project),
    DeleteStep::BigLakeConnection(id) => cold_storage::destroy_connection(id, &config.project, &config.region),
```

If the closure signature there doesn't currently have a borrow on `res`/`config`, capture them in the closure. Match the style of the existing branches.

Also add at the file's top: `use crate::lib::cold_storage;`

- [ ] **Step 5: Build + test**

```bash
cargo build
cargo test --lib
```
Expected: green. If any branch has a borrow/move conflict, follow rustc's hint.

- [ ] **Step 6: Commit**

```bash
git add src/commands/deploy.rs src/commands/destroy.rs
git commit -m "feat(deploy/destroy): wire cold tier into lifecycle"
```

---

## Task 10: Dashboard tiles for cold tier

**Files:**
- Modify: `src/lib/dashboard.rs`.

- [ ] **Step 1: Read enough of `dashboard.rs` to find the existing BQ-subscription tile and the helper function used to build tiles**

```bash
grep -n "bq_subscription\|fn build\|tile" src/lib/dashboard.rs | head -30
```

Determine the actual helper signature and tile layout. (The plan can't show exact code without that; the next step expects you to match the existing pattern.)

- [ ] **Step 2: Add two tiles next to the BQ subscription tile**

Pattern (substitute the actual helper name and arg shape you saw in Step 1):

```rust
// Cold-tier bytes (parquet under <bucket>/parquet/)
existing_tile_builder(
    "Cold tier (Parquet) bytes",
    &format!(
        r#"resource.type="gcs_bucket" AND metric.type="storage.googleapis.com/storage/total_bytes" AND resource.label.bucket_name="{}""#,
        res.bucket_name
    ),
)

// Daily export scheduled-query run count
existing_tile_builder(
    "Daily export job runs",
    &format!(
        r#"resource.type="bigquery_dts_config" AND metric.type="bigquerydatatransfer.googleapis.com/transfer_config/transfer_run_count" AND resource.label.config_id="{}""#,
        res.export_scheduled_query_id
    ),
)
```

If `res.export_scheduled_query_id` is the full resource name (`projects/.../transferConfigs/<id>`), trim to the trailing UUID for the `config_id` label.

- [ ] **Step 3: Build**

```bash
cargo build
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/dashboard.rs
git commit -m "feat(dashboard): cold-tier bytes + scheduled-query tiles"
```

---

## Task 11: README

**Files:**
- Modify: `README.md`.

- [ ] **Step 1: Append a Storage Tiers section**

```markdown
## Storage tiers

Beaver writes events into a **hot** BigQuery table (`<dataset>.<table>`,
day-partitioned by ingestion time with a 14-day partition expiration).
Daily, a BigQuery scheduled query rolls 13-day-old partitions out to
`gs://<bucket>/parquet/dt=YYYY-MM-DD/*.parquet` (zstd, Hive-partitioned)
and deletes them from the hot table. The cold prefix is exposed through a
BigLake external table (`<dataset>.events_cold`); the view
`<dataset>.events_all` unions hot + cold and exposes a unified `partition_date`
column.

GCS lifecycle: Standard → Nearline (30d) → Coldline (90d) → Archive (365d).

Tune retention/export age in `src/lib/cold_storage.rs` (constants at top).
The schema is unchanged: a single `data: JSON` column on both tiers.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: storage tiers (hot BQ + cold parquet/BigLake)"
```

---

## Self-Review

- **Spec coverage:** Hot retention (Task 1), cold storage (Tasks 3–6), unification (Task 7), automation (Task 8), wiring (Task 9), observability (Task 10), docs (Task 11). Pub/Sub + producers untouched (matches non-goals).
- **Placeholders:** Task 9 step 4(b) references "the closure passed to `execute`" without quoting code because the file structure isn't fully visible in the plan; the implementer must inspect `destroy.rs` to locate it. Acceptable since it's a one-time grep.
- **Type consistency:** field names (`biglake_connection_id`, `cold_bucket_prefix`, `cold_table_id`, `events_view_id`, `export_scheduled_query_id`) and helper signatures (`build_*_args`, `build_*_sql`) are consistent across tasks.

---

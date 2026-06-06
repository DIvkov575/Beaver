# Fix Code Review Bugs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three confirmed bugs from the code review: (1) dataflow staging bucket leaks on destroy, (2) duplicate/useless cold-tier dashboard tile, (3) stale sample config with broken `pubsub-in` reference.

**Architecture:** Each fix is independent. Bug 1 adds a `dataflow_staging_bucket` field to Resources + a DeleteStep so destroy cleans it up. Bug 2 removes the duplicate cold-tier tile (it's identical to the GCS bucket tile above it). Bug 3 fixes the embedded sample config YAML.

**Tech Stack:** Rust, serde_yaml, gcloud CLI

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/lib/resources.rs` | Add `dataflow_staging_bucket` field + `record_`/`forget_` methods |
| `src/commands/deploy.rs` | Record the staging bucket after creation |
| `src/commands/destroy.rs` | Add `DataflowStagingBucket` variant to `DeleteStep` + deletion logic |
| `src/lib/dashboard.rs` | Remove the duplicate cold-tier tile |
| `src/beaver_config/beaver_config.yaml` | Fix `pubsub-in` → `pubsub_in` and add `source` field |

---

### Task 1: Track dataflow staging bucket in resources.yaml

**Files:**
- Modify: `src/lib/resources.rs:10-50` (add field to `Resources`)
- Modify: `src/lib/resources.rs:52-83` (add to `empty()`)
- Modify: `src/lib/resources.rs:276-327` (add `record_`/`forget_` methods)

- [ ] **Step 1: Add the field to `Resources` struct**

In `src/lib/resources.rs`, add after line 49 (`export_sa_managed`):

```rust
    #[serde(default)]
    pub dataflow_staging_bucket: String,
```

- [ ] **Step 2: Initialize the field in `Resources::empty()`**

In the `Self { ... }` block, add after `export_sa_managed: false,`:

```rust
            dataflow_staging_bucket: String::new(),
```

- [ ] **Step 3: Add `record_dataflow_staging_bucket` and `forget_dataflow_staging_bucket` to Tracker**

In `src/lib/resources.rs`, add after the `forget_export_sa` method (before the closing `}` of `impl Tracker`):

```rust
    pub fn record_dataflow_staging_bucket(&mut self, name: String) -> Result<()> {
        self.res.dataflow_staging_bucket = name;
        self.persist()
    }
    pub fn forget_dataflow_staging_bucket(&mut self) -> Result<()> {
        self.res.dataflow_staging_bucket.clear();
        self.persist()
    }
```

- [ ] **Step 4: Run `cargo test resources` to verify compilation and existing tests pass**

Run: `cargo test resources -- --nocapture`
Expected: all existing resource tests pass (empty_resources_roundtrip, cold_tier_fields_roundtrip, etc.)

- [ ] **Step 5: Commit**

```bash
git add src/lib/resources.rs
git commit -m "fix(resources): add dataflow_staging_bucket field to Resources"
```

---

### Task 2: Record the staging bucket during deploy

**Files:**
- Modify: `src/commands/deploy.rs:88-113`

- [ ] **Step 1: Add `tracker.record_dataflow_staging_bucket()` call after bucket creation succeeds**

In `src/commands/deploy.rs`, the staging bucket block currently ends at the `grant_bucket` call. Add the tracker call right after the closing `}` of `if !exists.status.success() { ... }` (around line 112), before `service_accounts::grant_bucket`:

Replace:
```rust
        service_accounts::grant_bucket(&dataflow_staging, &sa.email, "roles/storage.objectAdmin")?;
```

With:
```rust
        tracker.record_dataflow_staging_bucket(dataflow_staging.clone())?;
        service_accounts::grant_bucket(&dataflow_staging, &sa.email, "roles/storage.objectAdmin")?;
```

- [ ] **Step 2: Run `cargo build` to verify compilation**

Run: `cargo build`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add src/commands/deploy.rs
git commit -m "fix(deploy): record dataflow staging bucket in resources.yaml"
```

---

### Task 3: Add destroy support for the staging bucket

**Files:**
- Modify: `src/commands/destroy.rs:10-32` (add enum variant)
- Modify: `src/commands/destroy.rs:39-114` (add to `plan()`)
- Modify: `src/commands/destroy.rs:119-185` (add to `execute` match + `dispatch_real`)

- [ ] **Step 1: Add `DataflowStagingBucket` variant to `DeleteStep` enum**

In `src/commands/destroy.rs`, add after line 31 (`ExportSa(String),`):

```rust
    DataflowStagingBucket(String),
```

- [ ] **Step 2: Add the staging bucket to `plan()`**

In the `plan()` function, add after the `GcsBucket` step (after line 101 `steps.push(DeleteStep::GcsBucket(...))`):

```rust
    if !res.dataflow_staging_bucket.is_empty() {
        steps.push(DeleteStep::DataflowStagingBucket(res.dataflow_staging_bucket.clone()));
    }
```

- [ ] **Step 3: Add the `forget` arm in `execute()`**

In the `execute` function's match block (around line 140), add after the `DeleteStep::GcsBucket(_) => tracker.forget_bucket(),` arm:

```rust
                    DeleteStep::DataflowStagingBucket(_) => tracker.forget_dataflow_staging_bucket(),
```

- [ ] **Step 4: Add the dispatch arm in `dispatch_real()`**

In `dispatch_real` (around line 171), add after the `DeleteStep::GcsBucket(name) => gcs::delete_bucket(name),` arm:

```rust
        DeleteStep::DataflowStagingBucket(name) => gcs::delete_bucket(name),
```

- [ ] **Step 5: Write a unit test that `plan()` includes the staging bucket**

Add to the `#[cfg(test)] mod tests` block in `destroy.rs`:

```rust
    #[test]
    fn plan_includes_dataflow_staging_bucket() {
        let mut res = empty_resources();
        res.dataflow_staging_bucket = "dataflow-staging-us-east1-123456".into();
        res.bucket_name = "beaver_abc".into();

        let steps = plan(&res);
        let staging_idx = steps.iter().position(|s| matches!(s, DeleteStep::DataflowStagingBucket(_))).unwrap();
        let main_idx = steps.iter().position(|s| matches!(s, DeleteStep::GcsBucket(_))).unwrap();
        assert!(staging_idx > main_idx, "staging bucket should be deleted after main bucket");
    }
```

- [ ] **Step 6: Run tests**

Run: `cargo test destroy -- --nocapture`
Expected: all destroy tests pass including the new one

- [ ] **Step 7: Commit**

```bash
git add src/commands/destroy.rs
git commit -m "fix(destroy): delete dataflow staging bucket on destroy"
```

---

### Task 4: Remove duplicate cold-tier dashboard tile

**Files:**
- Modify: `src/lib/dashboard.rs:320-335`

- [ ] **Step 1: Remove the cold-tier tile block and the `export_sq` suppression**

In `src/lib/dashboard.rs`, replace lines 320-335:

```rust
    // ---- Cold tier (Parquet) bytes: no-op (informational only).
    out.push(ComponentHealth {
        label: "Cold tier".to_string(),
        policy_name: create_health_policy(tracker, config,
            &format!("Beaver Cold tier ({}/parquet)", bucket),
            &noop_threshold(
                &format!(r#"resource.type="gcs_bucket" AND metric.type="storage.googleapis.com/storage/total_bytes" AND resource.label.bucket_name="{}""#, bucket),
                1e30,
            ))?,
    });

    // No dedicated "Export job" tile: the BigQuery Data Transfer Service does
    // not expose a per-config run-count metric Cloud Monitoring will accept in
    // an alert-policy filter. The Cold tier bytes tile already surfaces the
    // meaningful signal — parquet bytes growing means daily exports work.
    let _ = export_sq;
```

With:

```rust
    // Cold tier + Export job: removed. The GCS bucket tile above already shows
    // total bucket bytes (the metric is bucket-level, not prefix-level). BQ DTS
    // does not expose a per-config metric Cloud Monitoring accepts in an alert
    // filter, so no actionable tile can be created for the export job.
    let _ = export_sq;
```

- [ ] **Step 2: Run `cargo build` to confirm compilation**

Run: `cargo build`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add src/lib/dashboard.rs
git commit -m "fix(dashboard): remove duplicate cold-tier tile (identical to GCS bucket tile)"
```

---

### Task 5: Fix stale sample config

**Files:**
- Modify: `src/beaver_config/beaver_config.yaml`

- [ ] **Step 1: Fix the sample config**

Replace the entire contents of `src/beaver_config/beaver_config.yaml` with:

```yaml
beaver:
  project_id: neon-circle-400322
  region: northamerica-northeast1


sources:
  pubsub_in:
    type: gcp_pubsub
    project: "neon-circle-400322"
    subscription: "input1"
    decoding:
      codec: "json"

transforms:
  transform1:
    type: remap
    inputs:
      - pubsub_in
    source: |
      . = .

# Optional SOC triage dashboard (Cloud Monitoring). Uncomment to provision.
# dashboard:
#   enabled: true
#   name: "Beaver"
```

Key changes from the old file:
- `sources:` block no longer uses YAML list syntax (`- pubsub_in:`) — it uses a mapping key directly (`pubsub_in:`)
- `transforms.transform1.inputs` uses `pubsub_in` (underscore) instead of `pubsub-in` (hyphen)
- Added `source: |` + `. = .` (identity VRL, matches init.rs template)

- [ ] **Step 2: Run `cargo build` to confirm the embedded file still compiles**

Run: `cargo build`
Expected: compiles (the file is embedded via `include_bytes_zstd!` or read at runtime — either way the project must still build)

- [ ] **Step 3: Commit**

```bash
git add src/beaver_config/beaver_config.yaml
git commit -m "fix(config): correct stale pubsub-in reference in sample config"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Verify no new warnings related to our changes**

Run: `cargo build --release 2>&1 | grep -i "error"`
Expected: no errors (warnings about unused variables elsewhere are pre-existing and acceptable)

# Drain-Based Dataflow Reload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the cancel-based refresh-detections flow with a drain-based approach that flushes in-flight correlation windows before relaunching, preventing alert loss.

**Architecture:** Add `drain_job()` and `wait_for_drained()` to `src/lib/dataflow.rs` mirroring the existing `delete_job`/`wait_for_running` pattern. Update `refresh_detections` to drain by default, with `--cancel` as opt-in for the old destructive behavior. `repair-dataflow` gains the same flag so both commands can drain gracefully.

**Tech Stack:** Rust, clap (CLI parsing), gcloud CLI (Dataflow API)

---

### Task 1: Add `drain_job()` to `src/lib/dataflow.rs`

**Files:**
- Modify: `src/lib/dataflow.rs` (after `delete_job` at line ~341)

- [ ] **Step 1: Write the failing test**

```rust
// At the bottom of src/lib/dataflow.rs, add a test module:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // requires gcloud + active Dataflow job
    fn test_drain_job_no_active_job_is_ok() {
        let config = crate::lib::test_helpers::test_config();
        // A non-existent job should return Ok (no-op, same as delete_job)
        let result = drain_job("nonexistent-job-xyz-000", &config);
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test dataflow::tests::test_drain_job_no_active_job_is_ok -- --ignored 2>&1 | tail -20`
Expected: FAIL with "cannot find function `drain_job`"

- [ ] **Step 3: Write minimal implementation**

Add this function after `delete_job` in `src/lib/dataflow.rs`:

```rust
/// Drains the running Dataflow job. Unlike `delete_job` (cancel), drain tells
/// the job to stop reading new input but finish processing all in-flight
/// elements — correlation windows flush their accumulated state before the job
/// enters DRAINED. Pub/Sub messages arriving after the drain request queue
/// until the replacement job starts.
pub fn drain_job(name: &str, config: &Config) -> Result<()> {
    info!("draining dataflow job: {}", name);

    let lookup = Command::new("gcloud")
        .args([
            "dataflow", "jobs", "list",
            "--region", &config.region,
            "--project", &config.project,
            "--filter", &format!("name={}", name),
            "--format=value(JOB_ID)",
            "--status=active",
        ])
        .output()?;
    let job_id = String::from_utf8_lossy(&lookup.stdout).trim().to_string();
    if job_id.is_empty() {
        info!("no active dataflow job named {}; nothing to drain", name);
        return Ok(());
    }

    let drain = Command::new("gcloud")
        .args([
            "dataflow", "jobs", "drain", &job_id,
            "--region", &config.region,
            "--project", &config.project,
        ])
        .output()?;
    if !drain.status.success() {
        let stderr = String::from_utf8_lossy(&drain.stderr);
        return Err(anyhow!("dataflow drain failed: {}", stderr));
    }
    info!("drain request accepted for job {}", name);
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test dataflow::tests::test_drain_job_no_active_job_is_ok -- --ignored 2>&1 | tail -20`
Expected: PASS (returns Ok for non-existent job)

- [ ] **Step 5: Verify it compiles without --ignored**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles cleanly

- [ ] **Step 6: Commit**

```bash
git add src/lib/dataflow.rs
git commit -m "feat(dataflow): add drain_job() for graceful Dataflow shutdown"
```

---

### Task 2: Add `wait_for_drained()` to `src/lib/dataflow.rs`

**Files:**
- Modify: `src/lib/dataflow.rs` (after `drain_job`)

- [ ] **Step 1: Write the failing test**

```rust
// Add to the existing #[cfg(test)] mod tests block:
#[test]
#[ignore]
fn test_wait_for_drained_nonexistent_job_errors() {
    let config = crate::lib::test_helpers::test_config();
    let result = wait_for_drained("nonexistent-job-xyz-000", &config);
    assert!(result.is_err()); // can't find job → error
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test dataflow::tests::test_wait_for_drained -- --ignored 2>&1 | tail -20`
Expected: FAIL with "cannot find function `wait_for_drained`"

- [ ] **Step 3: Write minimal implementation**

Add this function after `drain_job`:

```rust
/// Polls until the Dataflow job reaches JOB_STATE_DRAINED (or a terminal
/// failure state). Timeout is generous (10 min) because draining a streaming
/// job with large windows can take several minutes as it flushes all pending
/// timers and in-flight bundles.
pub fn wait_for_drained(name: &str, config: &Config) -> Result<()> {
    info!("waiting for Dataflow job {} to reach Drained state", name);
    let job_id = lookup_job_id(name, config)?;
    let timeout = std::time::Duration::from_secs(600);
    let interval = std::time::Duration::from_secs(15);
    let start = std::time::Instant::now();
    let mut last_state = String::new();
    loop {
        let state = describe_state(&job_id, config)?;
        if state != last_state {
            info!("Dataflow state: {}", state);
            last_state = state.clone();
        }
        match state.as_str() {
            "JOB_STATE_DRAINED" => return Ok(()),
            "JOB_STATE_FAILED" | "JOB_STATE_CANCELLED" => {
                let errs = recent_errors(&job_id, config);
                return Err(anyhow!(
                    "Dataflow job {} entered {} while draining (expected DRAINED).\n\
                     Recent worker errors:\n{}",
                    name, state, errs
                ));
            }
            _ => {}
        }
        if start.elapsed() > timeout {
            let errs = recent_errors(&job_id, config);
            return Err(anyhow!(
                "Dataflow job {} did not reach DRAINED within {:?} (last state: {}).\n\
                 Recent worker errors:\n{}",
                name, timeout, state, errs
            ));
        }
        std::thread::sleep(interval);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test dataflow::tests::test_wait_for_drained -- --ignored 2>&1 | tail -20`
Expected: PASS (errors on nonexistent job as expected)

- [ ] **Step 5: Verify compilation**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles cleanly

- [ ] **Step 6: Commit**

```bash
git add src/lib/dataflow.rs
git commit -m "feat(dataflow): add wait_for_drained() with 10-min timeout"
```

---

### Task 3: Add `--cancel` flag to `RefreshDetections` CLI command

**Files:**
- Modify: `src/commands.rs` (the `RefreshDetections` variant)
- Modify: `src/commands/repair.rs` (`refresh_detections` function signature)

- [ ] **Step 1: Write the failing test**

```rust
// In src/commands.rs, this is a compilation test — the flag must parse.
// We verify by building with the new flag. Add a unit test at the bottom
// of src/commands.rs:
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn refresh_detections_parses_cancel_flag() {
        // Default (no --cancel) → cancel=false (drain mode)
        let cmd = Command::parse_from(["beaver", "refresh-detections", "--path", "/tmp/x"]);
        match cmd {
            Command::RefreshDetections { cancel, .. } => assert!(!cancel),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn refresh_detections_parses_cancel_flag_explicit() {
        let cmd = Command::parse_from(["beaver", "refresh-detections", "--path", "/tmp/x", "--cancel"]);
        match cmd {
            Command::RefreshDetections { cancel, .. } => assert!(cancel),
            _ => panic!("wrong variant"),
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commands::tests::refresh_detections_parses -- 2>&1 | tail -20`
Expected: FAIL — no `cancel` field on `RefreshDetections`

- [ ] **Step 3: Modify `RefreshDetections` enum variant to accept `--cancel`**

In `src/commands.rs`, change the `RefreshDetections` variant:

```rust
#[command(about="Recompile sigma rules and relaunch Dataflow with new detections")]
RefreshDetections {
    #[arg(short, long)]
    path: String,
    /// Use cancel (immediate kill) instead of drain (graceful flush).
    /// Drain is the default — it lets in-flight correlation windows complete
    /// before shutting down, preventing alert loss.
    #[arg(long, default_value_t = false)]
    cancel: bool,
},
```

Update the `run` match arm:

```rust
RefreshDetections{path, cancel} => refresh_detections(&path, cancel),
```

- [ ] **Step 4: Update `refresh_detections` signature**

In `src/commands/repair.rs`, change the signature:

```rust
pub fn refresh_detections(path_arg: &str, use_cancel: bool) -> Result<()> {
```

For now, keep the body unchanged (still calls `delete_job`). This makes it compile.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test commands::tests::refresh_detections_parses -- 2>&1 | tail -20`
Expected: PASS (both tests)

- [ ] **Step 6: Commit**

```bash
git add src/commands.rs src/commands/repair.rs
git commit -m "feat(cli): add --cancel flag to refresh-detections (drain is default)"
```

---

### Task 4: Wire drain logic into `refresh_detections`

**Files:**
- Modify: `src/commands/repair.rs` (the `refresh_detections` function body)

- [ ] **Step 1: Write the failing test**

This is an integration test — we verify via a compilation + logic review. The key assertion: when `use_cancel=false`, the function calls `drain_job` + `wait_for_drained` instead of `delete_job`.

Since these shell out to gcloud, we verify via the compilation path and a smoke assertion:

```rust
// Add to the #[cfg(test)] in repair.rs or dataflow.rs:
// This is a compile-time contract test — if the import is wrong, it fails.
#[cfg(test)]
mod tests {
    #[test]
    fn drain_path_compiles() {
        // Ensure the public API exists and is callable
        let _: fn(&str, &crate::lib::config::Config) -> anyhow::Result<()> =
            crate::lib::dataflow::drain_job;
        let _: fn(&str, &crate::lib::config::Config) -> anyhow::Result<()> =
            crate::lib::dataflow::wait_for_drained;
    }
}
```

- [ ] **Step 2: Implement the drain/cancel branching**

In `src/commands/repair.rs`, replace the cancel step in `refresh_detections`:

```rust
pub fn refresh_detections(path_arg: &str, use_cancel: bool) -> Result<()> {
    info!("=======Refreshing Detections======");
    println!("\nBeaver refresh-detections");
    println!("=========================\n");

    let path = Path::new(path_arg);
    validate_config_path(path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let resources_path = path.join("artifacts/resources.yaml");
    if !resources_path.exists() {
        return Err(anyhow!("resources.yaml not found at {}", resources_path.display()));
    }
    let yaml = std::fs::read_to_string(&resources_path)?;
    let mut resources: Resources = serde_yaml::from_str(&yaml)?;
    if let Ok(abs) = path.canonicalize() {
        resources.config_path = abs.as_os_str().to_str().unwrap().to_string();
    }
    let config = Config::from_path(path);

    let current_name = resources.dataflow_pipeline_name.clone();
    if current_name.is_empty() {
        return Err(anyhow!(
            "no Dataflow job recorded in resources.yaml; run `beaver deploy` first"
        ));
    }

    step("compile sigma rules", || {
        sigma::setup_detections_venv(path)?;
        sigma::generate_detections(path)?;
        detections_gen::generate_detections_file(path)
    })?;

    if use_cancel {
        let _ = step("cancel existing job (immediate)", || {
            dataflow::delete_job(&current_name, &config)
        });
    } else {
        step("drain existing job (flushing windows)", || {
            dataflow::drain_job(&current_name, &config)
        })?;
        step("wait for drain to complete (up to 10 min)", || {
            dataflow::wait_for_drained(&current_name, &config)
        })?;
    }

    let mut tracker = Tracker::new(&mut resources);

    step("re-upload Dataflow template", || {
        dataflow::create_template(path, &mut tracker, &config)
    })?;
    step("launch new Dataflow streaming job", || {
        dataflow::create_pipeline(&mut tracker, &config)
    })?;
    let new_name = tracker.resources().dataflow_pipeline_name.clone();
    step("wait for Dataflow workers to come up (up to 5 min)", || {
        dataflow::wait_for_running(&new_name, &config)
    })?;

    let mode = if use_cancel { "cancelled" } else { "drained" };
    println!(
        "\nRefreshed: old job {} {}, new job {} is Running with new detections.\n",
        current_name, mode, new_name
    );
    Ok(())
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles cleanly

- [ ] **Step 4: Run all unit tests**

Run: `cargo test 2>&1 | tail -20`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/commands/repair.rs
git commit -m "feat(refresh): use drain by default, --cancel for immediate kill"
```

---

### Task 5: Add `--cancel` flag to `RepairDataflow` for consistency

**Files:**
- Modify: `src/commands.rs` (the `RepairDataflow` variant)
- Modify: `src/commands/repair.rs` (`repair_dataflow` function)

- [ ] **Step 1: Add the flag to the CLI variant**

In `src/commands.rs`:

```rust
#[command(about="Relaunch the Dataflow job for an existing deploy")]
RepairDataflow {
    #[arg(short, long)]
    path: String,
    /// Use cancel instead of drain when stopping the existing job.
    #[arg(long, default_value_t = false)]
    cancel: bool,
},
```

Update the match arm:

```rust
RepairDataflow{path, cancel} => repair_dataflow(&path, cancel),
```

- [ ] **Step 2: Update `repair_dataflow` to accept and use the flag**

Change signature to `pub fn repair_dataflow(path_arg: &str, use_cancel: bool) -> Result<()>`.

Replace the existing cancel step (the `let _ = step("cancel existing job..."` block) with:

```rust
if use_cancel {
    let _ = step("cancel existing job (immediate)", || {
        dataflow::delete_job(&current_name, &config)
    });
} else {
    step("drain existing job (flushing windows)", || {
        dataflow::drain_job(&current_name, &config)
    })?;
    step("wait for drain to complete (up to 10 min)", || {
        dataflow::wait_for_drained(&current_name, &config)
    })?;
}
```

- [ ] **Step 3: Add CLI parse test**

```rust
#[test]
fn repair_dataflow_parses_cancel_flag() {
    let cmd = Command::parse_from(["beaver", "repair-dataflow", "--path", "/tmp/x"]);
    match cmd {
        Command::RepairDataflow { cancel, .. } => assert!(!cancel),
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo test 2>&1 | tail -20`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/commands.rs src/commands/repair.rs
git commit -m "feat(repair-dataflow): add --cancel flag, drain by default"
```

---

### Task 6: Update `repair_dataflow` to skip drain when job is already inactive

**Files:**
- Modify: `src/commands/repair.rs`

- [ ] **Step 1: Identify the edge case**

`repair_dataflow` already probes current state and returns early if RUNNING. But if the job is FAILED/CANCELLED/DRAINED, the drain call would fail (can't drain an inactive job). The current code has `let _ = step(...)` (ignores errors) — but with drain as default we need to handle this cleanly.

- [ ] **Step 2: Modify `repair_dataflow` to only drain/cancel if active**

After the state probe block (the `match state.as_deref()` that returns early for Running), replace the cancel step:

```rust
// Only attempt shutdown if the job is still active. Jobs in terminal
// states (Failed, Cancelled, Drained) don't need a stop command.
// current_state() returns short-form names: "Running", "Failed", etc.
// (via `gcloud dataflow jobs list --format=value(STATE)`)
let needs_shutdown = !matches!(
    state.as_deref(),
    Some("Failed") | Some("Cancelled") | Some("Drained")
);

if needs_shutdown {
    if use_cancel {
        let _ = step("cancel existing job (immediate)", || {
            dataflow::delete_job(&current_name, &config)
        });
    } else {
        step("drain existing job (flushing windows)", || {
            dataflow::drain_job(&current_name, &config)
        })?;
        step("wait for drain to complete (up to 10 min)", || {
            dataflow::wait_for_drained(&current_name, &config)
        })?;
    }
}
```

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo test 2>&1 | tail -20`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/commands/repair.rs
git commit -m "fix(repair): skip drain/cancel for already-inactive jobs"
```

---

### Task 7: Final integration verification

**Files:**
- None modified (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test 2>&1 | tail -30`
Expected: all tests pass, no warnings

- [ ] **Step 2: Verify CLI help output**

Run: `cargo run -- refresh-detections --help 2>&1`
Expected: shows `--cancel` flag with description about drain being default

Run: `cargo run -- repair-dataflow --help 2>&1`
Expected: shows `--cancel` flag

- [ ] **Step 3: Verify binary builds in release mode**

Run: `cargo build --release 2>&1 | tail -5`
Expected: compiles cleanly

- [ ] **Step 4: Commit (if any formatting/clippy fixes needed)**

```bash
# Only if changes were needed:
git add -A
git commit -m "chore: clippy/fmt cleanup for drain feature"
```

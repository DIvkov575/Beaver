# Drain-Based Reload — GCP Validation Plan

**Commit:** `b8a508b` (main)  
**Feature:** `beaver refresh-detections` and `repair-dataflow` now drain by default instead of cancel  
**Status:** Code complete, unit tests pass (49/49), code-reviewed and bug-fixed. Needs live GCP validation.

---

## Prerequisites

- Deployed Beaver pipeline (`beaver deploy --path <config_dir>`) with a running Dataflow job
- At least one correlation rule active (e.g., `06_correlation_brute_force.yml` — 5 failed logins in 10 min)
- Pub/Sub input topic receiving events (or ability to publish test events)
- `gcloud` authenticated with project access

---

## Test 1: Happy Path — Drain + Relaunch

**Goal:** Verify the default drain flow works end-to-end.

```bash
# 1. Confirm current job is Running
gcloud dataflow jobs list --region=<REGION> --project=<PROJECT> --status=active

# 2. Run refresh with drain (default)
beaver refresh-detections --path <config_dir>

# 3. Expected output sequence:
#    ✓ compile sigma rules
#    ✓ drain existing job (flushing windows)      ← NEW
#    ✓ wait for drain to complete (up to 10 min)  ← NEW
#    ✓ re-upload Dataflow template
#    ✓ launch new Dataflow streaming job
#    ✓ wait for Dataflow workers to come up (up to 5 min)
#    Refreshed: old job <old> drained, new job <new> is Running with new detections.

# 4. Verify old job reached DRAINED
gcloud dataflow jobs list --region=<REGION> --project=<PROJECT> \
  --filter="name=<old_job_name>" --format="value(STATE)"
# Expected: Drained

# 5. Verify new job is Running
gcloud dataflow jobs list --region=<REGION> --project=<PROJECT> --status=active
# Expected: new job name present, state=Running
```

**Pass criteria:** Old job state = Drained, new job state = Running, no errors in output.

---

## Test 2: Correlation Window Flush During Drain

**Goal:** Verify that in-flight correlation state fires alerts before the drain completes.

```bash
# 1. Publish 4 failed-login events (brute-force rule needs 5 in 10 min)
for i in 1 2 3 4; do
  gcloud pubsub topics publish <input_topic> --project=<PROJECT> \
    --message='{"EventID":4625,"User":"testuser","SourceIP":"10.0.0.1","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"}'
  sleep 1
done

# 2. Wait ~30s for events to enter the correlation window

# 3. Publish the 5th event (should trigger alert)
gcloud pubsub topics publish <input_topic> --project=<PROJECT> \
  --message='{"EventID":4625,"User":"testuser","SourceIP":"10.0.0.1","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"}'

# 4. Immediately trigger drain
beaver refresh-detections --path <config_dir>

# 5. Check BigQuery or Cloud Logging for the correlation alert
#    The alert should fire DURING the drain (window flush), not be lost
bq query --use_legacy_sql=false \
  'SELECT * FROM `<PROJECT>.<DATASET>.alerts` WHERE rule_id LIKE "%brute_force%" ORDER BY fired_at DESC LIMIT 5'
```

**Pass criteria:** The brute-force alert appears in BQ/logs despite the drain happening immediately after the 5th event. The correlation window flushed before the job reached DRAINED.

---

## Test 3: --cancel Flag (Backward Compatibility)

**Goal:** Verify `--cancel` still works as the old immediate-kill behavior.

```bash
# 1. With a running job:
beaver refresh-detections --path <config_dir> --cancel

# 2. Expected output:
#    ✓ compile sigma rules
#    ✓ cancel existing job (immediate)
#    ✓ re-upload Dataflow template
#    ✓ launch new Dataflow streaming job
#    ✓ wait for Dataflow workers to come up (up to 5 min)
#    Refreshed: old job <old> cancelled, new job <new> is Running with new detections.

# 3. Verify old job state
gcloud dataflow jobs list --region=<REGION> --project=<PROJECT> \
  --filter="name=<old_job_name>" --format="value(STATE)"
# Expected: Cancelled
```

**Pass criteria:** Immediate kill, no drain wait, old job = Cancelled.

---

## Test 4: Already-Stopped Job (No Crash)

**Goal:** Verify the command doesn't crash when the recorded job is already dead.

```bash
# 1. Manually cancel the current job
gcloud dataflow jobs cancel <JOB_ID> --region=<REGION> --project=<PROJECT>

# 2. Wait for it to reach Cancelled state

# 3. Run refresh — should skip drain entirely
beaver refresh-detections --path <config_dir>

# 4. Expected: no "drain existing job" step appears, goes straight to template upload
```

**Pass criteria:** Command succeeds, skips drain/cancel, launches new job without errors.

---

## Test 5: Non-Existent Job (Deleted from GCP)

**Goal:** Verify the command handles a job name in resources.yaml that no longer exists on GCP.

```bash
# 1. Edit resources.yaml to have a fake job name
#    dataflow_pipeline_name: "beaver-detections-fakefake"

# 2. Run refresh
beaver refresh-detections --path <config_dir>

# 3. Expected: skips drain (state lookup returns None → needs_shutdown=false),
#    proceeds to template upload + launch

# 4. Restore resources.yaml afterward
```

**Pass criteria:** No crash, no "could not find job" error, proceeds to relaunch.

---

## Test 6: repair-dataflow with Drain

**Goal:** Verify `repair-dataflow` also drains correctly.

```bash
# 1. Manually stop the running job (simulate a failure)
gcloud dataflow jobs cancel <JOB_ID> --region=<REGION> --project=<PROJECT>

# 2. Run repair
beaver repair-dataflow --path <config_dir>

# 3. Expected: detects Cancelled state, skips drain, relaunches
#    current state: Cancelled — relaunching
#    ✓ re-upload Dataflow template
#    ✓ launch new Dataflow streaming job
#    ...
```

**Pass criteria:** Correctly identifies the dead job, skips shutdown, relaunches.

---

## Test 7: Drain Timeout (Edge Case)

**Goal:** Verify behavior when drain takes longer than expected (10-min timeout).

This is hard to trigger intentionally. Monitor for:
- Jobs with very large windows (hours) or high allowed_lateness
- If timeout hits, the error message should include recent worker errors from Cloud Logging

**Verification:** Run drain on a job with a large window config and observe timing. If it times out, the error should be actionable (not a generic "timeout").

---

## Cleanup Checklist

After all tests:
- [ ] Ensure a healthy Running Dataflow job remains
- [ ] Verify `resources.yaml` reflects the latest job name
- [ ] Check Cloud Monitoring dashboard still shows active metrics
- [ ] No orphaned Dataflow jobs left in RUNNING state (check `--status=active`)

---

## Known Limitations

1. **Correlation window continuity across drain:** The drain flushes existing windows, but the NEW job starts with empty state. A user with 4/5 failed logins in the old job's window won't carry that count to the new job — they'd need all 5 to happen again in the new job's window. This is inherent to Dataflow (no state migration between jobs).

2. **Pub/Sub gap:** Between drain completion and new job reaching RUNNING (~2-3 min), Pub/Sub messages queue. They're not lost (subscription retains them), but there's a processing delay. Correlation windows that span this gap may miss events at the boundary.

3. **Drain duration:** Streaming jobs with `allowed_lateness_seconds=300` (default) may take up to 5 minutes to drain as they wait for late-arriving events to fire their windows. Jobs with larger lateness settings take proportionally longer.

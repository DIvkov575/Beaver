# GCP Validation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate the sigma-beam integration end-to-end on real GCP infrastructure — verify pickle serialization, windowed correlation, dashboard rendering, and alert delivery.

**Architecture:** Deploy Beaver's test pipeline to `neon-circle-400322`, publish synthetic events, observe alerts landing in BigQuery and dashboard tiles rendering. Fix any issues discovered. This is a validation pass, not new feature work.

**Tech Stack:** GCP (Pub/Sub, Dataflow, BigQuery, Cloud Monitoring), Rust CLI, Python 3.11, gcloud CLI.

**Prerequisites:** `gcloud auth login` with access to `neon-circle-400322`, `bq` CLI available.

---

## File Structure

| File | Purpose |
|------|---------|
| `src/lib/dashboard.rs` | Fix DLQ tile metric (known bug) |
| `sigma_beam/tests/test_pickle_roundtrip.py` | NEW — verify predicates survive cloudpickle |
| `sigma_beam/tests/test_windowed_correlation.py` | NEW — TestStream-based windowing tests |
| `examples/test-pipeline/beaver_config/detections/input/06_correlation_brute_force.yml` | NEW — correlation rule for e2e test |
| `examples/test-pipeline/beaver_config/placeholders.yaml` | NEW — placeholder table for deploy |

---

### Task 1: Fix DLQ Dashboard Tile (Known Bug)

**Files:**
- Modify: `src/lib/dashboard.rs`

The DLQ tile uses `resource.type="pubsub_subscription"` but the DLQ topic has no subscription — it's write-only. Must use topic-level metrics instead.

- [ ] **Step 1: Find and fix the DLQ tile filter**

In `src/lib/dashboard.rs`, find:
```rust
                  filter: 'resource.type="pubsub_subscription" AND resource.labels.subscription_id:"beaver-dlq" AND metric.type="pubsub.googleapis.com/subscription/num_undelivered_messages"'
```

Replace with:
```rust
                  filter: 'resource.type="pubsub_topic" AND resource.labels.topic_id="beaver-dlq" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count"'
```

Also change the aggregation from `ALIGN_MEAN` to `ALIGN_RATE` (message rate, not depth — since there's no subscription to accumulate):
```rust
                    perSeriesAligner: ALIGN_RATE
```

- [ ] **Step 2: Build + test**

```bash
cargo build && cargo test 2>&1 | tail -3
```
Expected: 49 passed.

- [ ] **Step 3: Commit**

```bash
git add src/lib/dashboard.rs && git commit -m "fix(dashboard): DLQ tile uses topic-level metrics (no subscription exists)"
```

---

### Task 2: Add Pickle Roundtrip Test

**Files:**
- Create: `sigma_beam/tests/test_pickle_roundtrip.py`

This verifies that `CompiledRule` objects (with real `compile_rule` predicates) survive cloudpickle serialization — the exact operation Dataflow performs when shipping transforms to workers.

- [ ] **Step 1: Install cloudpickle in venv**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam
.venv/bin/pip install cloudpickle
```

- [ ] **Step 2: Write the test**

```python
# sigma_beam/tests/test_pickle_roundtrip.py
"""Verify that CompiledRule predicates survive cloudpickle roundtrip.

Dataflow serializes all DoFns/lambdas via cloudpickle to ship to workers.
If any predicate closure captures a non-picklable object, the job fails
at graph construction time. This test catches that before deployment.
"""
import pickle
from pathlib import Path

import cloudpickle

from sigma_beam.loader import load_from_dir

FIXTURES = Path(__file__).parent / "fixtures" / "rules"


def test_single_event_rules_pickle_roundtrip():
    """Every compiled single-event rule must survive pickle roundtrip."""
    rs = load_from_dir(FIXTURES)
    for rule in rs.single_event:
        data = cloudpickle.dumps(rule)
        restored = pickle.loads(data)
        # Verify the predicate still works after deserialization
        assert restored.id == rule.id
        assert restored.title == rule.title
        # Test with a matching event
        assert restored.predicate({"EventID": 4625}) == rule.predicate({"EventID": 4625})
        # Test with a non-matching event
        assert restored.predicate({"EventID": 9999}) == rule.predicate({"EventID": 9999})


def test_correlation_rules_pickle_roundtrip():
    """CompiledCorrelation (no predicate, just data) must pickle."""
    rs = load_from_dir(FIXTURES)
    for corr in rs.correlation:
        data = cloudpickle.dumps(corr)
        restored = pickle.loads(data)
        assert restored.id == corr.id
        assert restored.kind == corr.kind


def test_lambda_over_refs_pickles():
    """The lambda `passes_any_referenced_rule(e, refs)` pattern must pickle."""
    from sigma_beam.correlation._common import passes_any_referenced_rule

    rs = load_from_dir(FIXTURES)
    refs = list(rs.single_event)
    # This is exactly what correlation transforms do:
    fn = lambda e: passes_any_referenced_rule(e, refs)
    data = cloudpickle.dumps(fn)
    restored_fn = pickle.loads(data)
    assert restored_fn({"EventID": 4625}) == fn({"EventID": 4625})
    assert restored_fn({"EventID": 9999}) == fn({"EventID": 9999})
```

- [ ] **Step 3: Run**

```bash
.venv/bin/python -m pytest tests/test_pickle_roundtrip.py -v
```
Expected: PASS. If it FAILS, we've found the Dataflow-breaking bug before deployment.

- [ ] **Step 4: Commit**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam
git add tests/test_pickle_roundtrip.py && git commit -m "test: add pickle roundtrip test for Dataflow serialization safety"
```

---

### Task 3: Add TestStream-Based Windowing Tests

**Files:**
- Create: `sigma_beam/tests/test_windowed_correlation.py`

These use Beam's `TestStream` to control event-time precisely, testing: window boundaries, late data, and allowed_lateness.

- [ ] **Step 1: Write windowed event_count test**

```python
# sigma_beam/tests/test_windowed_correlation.py
"""Windowed correlation tests using TestStream for real event-time semantics."""
from datetime import datetime, timezone

import apache_beam as beam
from apache_beam.testing.test_pipeline import TestPipeline
from apache_beam.testing.test_stream import TestStream
from apache_beam.testing.util import assert_that, equal_to, is_not_empty
from apache_beam.transforms.window import TimestampedValue
from apache_beam.utils.timestamp import Timestamp

from sigma_beam.correlation.event_count import EventCountCorrelation
from sigma_beam.correlation.temporal import TemporalCorrelation
from sigma_beam.ruleset import CompiledCorrelation, CompiledRule


def _ts(epoch_s: int) -> Timestamp:
    return Timestamp(seconds=epoch_s)


def _always_true(e: dict) -> bool:
    return True


def _matches_k(k: str):
    def pred(e: dict) -> bool:
        return e.get("k") == k
    return pred


def test_event_count_fires_within_window():
    """3 events in a 60s window with threshold=3 → 1 alert."""
    corr = CompiledCorrelation(
        id="c1", title="count", severity="high", kind="event_count",
        referenced_rule_ids=("r1",), group_by=("user",),
        window_seconds=60, threshold=3, threshold_op="gte",
    )
    refs = [CompiledRule(id="r1", title="r", severity="low", predicate=_always_true)]

    # All 3 events at t=10, t=20, t=30 (within same 0-60 window)
    ts = (
        TestStream()
        .add_elements([
            TimestampedValue({"user": "alice", "timestamp": 10}, _ts(10)),
            TimestampedValue({"user": "alice", "timestamp": 20}, _ts(20)),
            TimestampedValue({"user": "alice", "timestamp": 30}, _ts(30)),
        ])
        .advance_watermark_to_infinity()
    )

    with TestPipeline() as p:
        events = p | ts
        alerts = events | EventCountCorrelation(corr, refs)
        assert_that(alerts, is_not_empty())


def test_event_count_does_not_fire_across_windows():
    """2 events in window [0,60) + 1 in [60,120) with threshold=3 → 0 alerts."""
    corr = CompiledCorrelation(
        id="c1", title="count", severity="high", kind="event_count",
        referenced_rule_ids=("r1",), group_by=("user",),
        window_seconds=60, threshold=3, threshold_op="gte",
    )
    refs = [CompiledRule(id="r1", title="r", severity="low", predicate=_always_true)]

    ts = (
        TestStream()
        .add_elements([
            TimestampedValue({"user": "alice", "timestamp": 10}, _ts(10)),
            TimestampedValue({"user": "alice", "timestamp": 20}, _ts(20)),
        ])
        .advance_watermark_to(_ts(60))
        .add_elements([
            TimestampedValue({"user": "alice", "timestamp": 70}, _ts(70)),
        ])
        .advance_watermark_to_infinity()
    )

    with TestPipeline() as p:
        events = p | ts
        alerts = events | EventCountCorrelation(corr, refs)
        assert_that(alerts, equal_to([]))


def test_temporal_fires_when_both_rules_in_window():
    """Events matching rule A and rule B in same window → alert."""
    corr = CompiledCorrelation(
        id="c1", title="ab", severity="high", kind="temporal",
        referenced_rule_ids=("A", "B"), group_by=("user",),
        window_seconds=60,
    )
    refs = [
        CompiledRule(id="A", title="a", severity="low", predicate=_matches_k("a")),
        CompiledRule(id="B", title="b", severity="low", predicate=_matches_k("b")),
    ]

    ts = (
        TestStream()
        .add_elements([
            TimestampedValue({"user": "alice", "k": "a", "timestamp": 5}, _ts(5)),
            TimestampedValue({"user": "alice", "k": "b", "timestamp": 15}, _ts(15)),
        ])
        .advance_watermark_to_infinity()
    )

    with TestPipeline() as p:
        events = p | ts
        alerts = events | TemporalCorrelation(corr, refs)
        assert_that(alerts, is_not_empty())


def test_temporal_does_not_fire_across_window_boundary():
    """Rule A in window [0,60) and rule B in [60,120) → no alert."""
    corr = CompiledCorrelation(
        id="c1", title="ab", severity="high", kind="temporal",
        referenced_rule_ids=("A", "B"), group_by=("user",),
        window_seconds=60,
    )
    refs = [
        CompiledRule(id="A", title="a", severity="low", predicate=_matches_k("a")),
        CompiledRule(id="B", title="b", severity="low", predicate=_matches_k("b")),
    ]

    ts = (
        TestStream()
        .add_elements([
            TimestampedValue({"user": "alice", "k": "a", "timestamp": 5}, _ts(5)),
        ])
        .advance_watermark_to(_ts(60))
        .add_elements([
            TimestampedValue({"user": "alice", "k": "b", "timestamp": 65}, _ts(65)),
        ])
        .advance_watermark_to_infinity()
    )

    with TestPipeline() as p:
        events = p | ts
        alerts = events | TemporalCorrelation(corr, refs)
        assert_that(alerts, equal_to([]))
```

- [ ] **Step 2: Run**

```bash
.venv/bin/python -m pytest tests/test_windowed_correlation.py -v
```
Expected: PASS. If windowing is broken, these will catch it.

- [ ] **Step 3: Commit**

```bash
git add tests/test_windowed_correlation.py && git commit -m "test: add TestStream-based windowing tests for event_count + temporal"
```

---

### Task 4: Add Correlation Rule to Test Pipeline

**Files:**
- Create: `examples/test-pipeline/beaver_config/detections/input/06_correlation_brute_force.yml`

Add a correlation rule so the e2e deploy exercises the full correlation path.

- [ ] **Step 1: Create the correlation rule**

```yaml
# examples/test-pipeline/beaver_config/detections/input/06_correlation_brute_force.yml
---
title: brute_force_5_in_60s
id: c0c0c100-0001-4000-9000-000000000001
type: event_count
status: experimental
rules:
  - a1f1f100-0001-4000-9000-000000000001
group-by:
  - src_ip
timespan: 1m
condition:
  gte: 5
level: high
```

This references `01_failed_login.yml` (id `a1f1f100-0001-4000-9000-000000000001`). When 5+ failed login events from the same `src_ip` arrive within 60s, an alert fires.

- [ ] **Step 2: Verify it loads cleanly**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam
.venv/bin/python -c "
from sigma_beam.loader import load_from_dir
rs = load_from_dir('../examples/test-pipeline/beaver_config/detections/input')
print(f'{len(rs.single_event)} single-event, {len(rs.correlation)} correlation')
for c in rs.correlation:
    print(f'  {c.id}: {c.kind} threshold={c.threshold}')
"
```
Expected: 5 single-event, 1 correlation.

- [ ] **Step 3: Commit**

```bash
cd /Users/divkov/workplace/Beaver
git add examples/test-pipeline/beaver_config/detections/input/06_correlation_brute_force.yml
git commit -m "feat(examples): add brute_force correlation rule for e2e testing"
```

---

### Task 5: Deploy to GCP + Validate Pipeline

**Files:** None (operational validation)

This task deploys the full pipeline and validates it end-to-end.

- [ ] **Step 1: Authenticate**

```bash
gcloud auth login
gcloud config set project neon-circle-400322
```

- [ ] **Step 2: Deploy**

```bash
cd /Users/divkov/workplace/Beaver
cargo build --release
./target/release/beaver deploy --path examples/test-pipeline/beaver_config
```

Expected: ~18 steps complete, all green. Watch for:
- "compile sigma rules" — should stage 6 rules (5 single + 1 correlation)
- "upload Sigma rules to GCS" — should upload 6 YAMLs
- "upload Dataflow template" — this is where pickle serialization happens
- "launch Dataflow streaming job" — job starts
- "wait for Dataflow workers to come up" — reaches RUNNING state

If the template build fails with a pickle error, that's the serialization bug from the review — log the exact error and fix.

- [ ] **Step 3: Publish synthetic events**

```bash
# 7 failed logins from same IP within 30s → triggers brute_force correlation (threshold=5)
for i in $(seq 1 7); do
  gcloud pubsub topics publish beaver-test-pipeline-input \
    --project neon-circle-400322 \
    --message "{\"event\":\"login\",\"status\":\"failed\",\"src_ip\":\"10.0.0.99\",\"user\":\"admin\",\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}"
  sleep 2
done
```

- [ ] **Step 4: Verify single-event alerts land in BQ**

Wait 90 seconds for Dataflow processing, then:

```bash
bq query --project_id=neon-circle-400322 --use_legacy_sql=false \
  "SELECT rule_id, rule_title, fired_at, correlation_key
   FROM beaver_siem.alerts
   ORDER BY fired_at DESC
   LIMIT 10"
```

Expected: At least one row with `rule_id` matching `a1f1f100-0001-4000-9000-000000000001` (failed_login single-event detection). If correlation worked, also a row for `c0c0c100-0001-4000-9000-000000000001` (brute_force_5_in_60s).

- [ ] **Step 5: Verify DLQ receives malformed messages**

```bash
# Publish a non-JSON message
gcloud pubsub topics publish beaver-test-pipeline-input \
  --project neon-circle-400322 \
  --message "this is not json"
```

Wait 30s, then check DLQ topic received it:
```bash
gcloud pubsub subscriptions pull beaver-dlq-verify-sub \
  --project neon-circle-400322 --limit 5 --auto-ack 2>/dev/null || \
  echo "No DLQ subscription exists — check topic message count in console"
```

If no subscription exists on the DLQ topic, check in the console that the topic's "Messages published" metric shows the message.

- [ ] **Step 6: Check dashboard renders**

Open: `https://console.cloud.google.com/monitoring/dashboards?project=neon-circle-400322`

Verify:
- "Alerts / min (sigma_beam)" tile shows non-zero data after step 3
- "DLQ Depth" tile shows the malformed message
- No blank gaps in the layout

- [ ] **Step 7: Record findings**

If everything works: note it in `docs/sigma-beam-integration-handoff.md` under a new "## GCP Validation Results" section.

If something breaks: document the exact error, fix it, and re-deploy.

---

### Task 6: Destroy Test Resources

**Files:** None (operational cleanup)

- [ ] **Step 1: Destroy the deployment**

```bash
./target/release/beaver destroy --path examples/test-pipeline/beaver_config
```

Expected: All resources deleted, `resources.yaml` removed.

- [ ] **Step 2: Verify no leaked resources**

```bash
# Check no orphaned Dataflow jobs
gcloud dataflow jobs list --region us-east1 --project neon-circle-400322 --status=active

# Check no orphaned topics
gcloud pubsub topics list --project neon-circle-400322 | grep beaver

# Check no orphaned BQ datasets
bq ls --project_id neon-circle-400322 | grep beaver
```

Expected: All empty/no matches.

---

### Task 7: Install and Verify Sysmon Pipeline Test

**Files:**
- Modify: `sigma_beam/tests/test_pipelines.py` (only if test expectation is wrong)

- [ ] **Step 1: Install the plugin**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam
.venv/bin/pip install pySigma-pipeline-sysmon
```

- [ ] **Step 2: Run the previously-skipped test**

```bash
.venv/bin/python -m pytest tests/test_pipelines.py -v
```

If it passes: great, remove the `skipif` decorator and commit.

If it fails: the sysmon pipeline's current version doesn't inject EventID. Investigate:
```bash
.venv/bin/python -c "
from sigma.pipelines.sysmon import sysmon_pipeline
from sigma.rule import SigmaRule
import textwrap
p = sysmon_pipeline()
r = SigmaRule.from_yaml(textwrap.dedent('''
    title: test
    logsource:
        product: windows
        category: process_creation
    detection:
        sel: {Image|endswith: powershell.exe}
        condition: sel
'''))
p.apply(r)
print('Detection items after pipeline:')
for name, det in r.detection.detections.items():
    print(f'  {name}: {det}')
")
```

Based on the output, either fix the test expectation or fix the `default_selector` mapping in `processing.py`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "test(pipelines): verify sysmon pipeline test with plugin installed"
```

---

### Task 8: Final Test Suite Run + Push

- [ ] **Step 1: Run full sigma_beam suite**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam
.venv/bin/python -m pytest tests/ -v --ignore=tests/test_corpus_sigmahq.py
```
Expected: All pass, 0 skipped (sysmon test now runs with plugin installed).

- [ ] **Step 2: Run Rust suite**

```bash
cd /Users/divkov/workplace/Beaver && cargo test
```
Expected: 49 passed.

- [ ] **Step 3: Push everything**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && git push origin main
cd /Users/divkov/workplace/Beaver && git add sigma_beam && git commit -m "chore: update sigma_beam submodule after GCP validation" && git push origin main
```

---

## Success Criteria

All of these must be true for validation to pass:

- [ ] Pickle roundtrip test passes (Task 2)
- [ ] TestStream windowing tests pass (Task 3)
- [ ] Dataflow template builds without pickle error (Task 5, Step 2)
- [ ] Dataflow job reaches RUNNING state (Task 5, Step 2)
- [ ] Single-event alerts appear in BQ `alerts` table within 90s (Task 5, Step 4)
- [ ] Correlation alert (brute_force) appears in BQ (Task 5, Step 4)
- [ ] Malformed message reaches DLQ topic (Task 5, Step 5)
- [ ] Dashboard tiles render with data (Task 5, Step 6)
- [ ] Destroy cleans up all resources (Task 6)
- [ ] Sysmon pipeline test resolved (Task 7)
- [ ] Full test suite green (Task 8)

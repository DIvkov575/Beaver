# sigma-beam Integration Handoff

> **GCP validation (2026-06-07):** A live deploy to `neon-circle-400322` surfaced and
> fixed six deploy/runtime bugs: gcloud flag, stale Dataflow entrypoint, committed broken
> venv symlink, uninvoked venv setup, runtime-vs-compile-time `CARGO_MANIFEST_DIR`, and
> sigma_beam not shipped to Dataflow workers (`--extra_packages`).
> Full write-up: [`docs/gcp-validation-results.md`](gcp-validation-results.md).

## What Was Done

Integrated `DIvkov575/sigma-beam-backend` into Beaver as a git submodule at `./sigma_beam`. Added three features to the sigma_beam Python package, wired them into Beaver's deploy pipeline, and fixed all test failures.

### Commits (Beaver repo, main branch)

```
bf55133 chore: update sigma_beam submodule (all tests green)
94f22d9 chore: update sigma_beam submodule with review fixes
a236b8d fix(percentile): handle None values + fix merge_accumulators truncation
8ec73db fix(dashboard): remove 16-unit gap before sigma_beam tiles
c182fe1 chore: remove output.log, update sigma_beam submodule pointer
3d0c491 feat(dashboard): add sigma_beam alerts + DLQ tiles
3694c26 chore: pin sigma-beam-backend as git submodule
```

### Commits (sigma_beam submodule, main branch)

```
556168d fix(tests): skip sysmon pipeline test when plugin not installed
9b1e1ec fix(tests): resolve all 14 test failures (fixture isolation, Beam types, fieldref, temporal)
a7476fb test(nested): document supported group_by fields for nested correlations
ebe8594 fix(ruleset): correlations_by_id includes nested; log nested count
94f4689 fix(fanout): warn when nested correlations have no first-level input
b47a2c6 docs: update feature table for expand, nested, percentile
bb92206 test: e2e smoke tests for expand, nested, and percentile
b115d99 feat: wire percentile into loader + fanout dispatch
fbf3705 feat: percentile correlation PTransform
3065967 feat(correlation): nested fanout with AlertToEvent adapter
bf9a02e feat(loader): two-pass compilation with nested detection + cycle guard
b4b8666 feat(ruleset): add nested_correlation, correlations_by_id, percentile fields
cb068f7 feat: wire placeholders into loader + safety net in conditions
e647d0d feat: add |expand placeholder resolution module
```

### Features Added

| Feature | Status | Key files |
|---------|--------|-----------|
| `\|expand` placeholder substitution | Complete | `src/sigma_beam/placeholders.py`, `loader.py` |
| Nested correlation (correlation-of-correlations) | Complete (1 level) | `correlation/nested.py`, `correlation/fanout.py`, `loader.py` |
| Percentile aggregation (Beaver extension) | Complete | `correlation/percentile.py`, `loader.py` |
| Dashboard tiles (alerts/min, DLQ depth) | Complete | `src/lib/dashboard.rs` |
| Git submodule pinning | Complete | `.gitmodules` |

### Test Suite Status

```
sigma_beam: 120 passed, 1 skipped, 0 failed
Beaver Rust: 49 passed, 0 failed, 11 ignored (integration tests requiring GCP)
```

---

## Not Pushed

Neither repository has been pushed to remote.

```bash
# To push:
cd /Users/divkov/workplace/Beaver/sigma_beam && git push origin main
cd /Users/divkov/workplace/Beaver && git push origin main
```

---

## Testing Workarounds (No GCP Access)

All Python tests run on Apache Beam's **DirectRunner** — an in-process single-machine runner. This was the only option without GCP credentials. The implications:

### What DirectRunner does NOT exercise

| Concern | Why it matters | What would break |
|---------|---------------|------------------|
| Pickle serialization | Dataflow serializes all DoFns/transforms via cloudpickle to ship to workers. DirectRunner runs in-process — no serialization. | Lambdas in `percentile.py:100`, `temporal.py:84`, `temporal_ordered.py:111`, and `fanout.py:53` close over `refs: list[CompiledRule]` where each `CompiledRule.predicate` is a closure from `compile_rule`. If any predicate captures a non-picklable object, the Dataflow job fails at graph construction time. |
| Real event-time windowing | `beam.Create()` assigns elements to GlobalWindow with `Timestamp.MIN_VALUE`. Our `attach_event_time` wraps elements with proper timestamps, but DirectRunner advances the watermark instantly past all timestamps. | `allowed_lateness` is never tested. Late data drops are never tested. Cross-window splits (elements near window boundary) are never tested. All windowed tests pass because all elements land in a single window. |
| Multi-worker state merging | DirectRunner uses a single worker. `CombineFn.merge_accumulators` is called but without the parallelism/retry semantics of Dataflow. | The `_PercentileCombineFn` accumulator mutation pattern (in-place `append`) is safe only if Beam never aliases accumulator references. On Dataflow with speculative execution or retries, this could produce corrupt state — though in practice Beam checkpoints and deserializes accumulators between retries. |
| Pub/Sub I/O | `correlation_pipeline.py` uses `ReadFromPubSub`/`WriteToPubSub`. Tests use `beam.Create` instead. | The pipeline entrypoint is tested structurally but not end-to-end with real message flow. |
| GCS rule loading | `load_from_gcs` is tested only via `load_from_dir` (the GCS code downloads to a temp dir, then calls `load_from_dir`). | Blob listing, download errors, and partial uploads are untested. |

### What DirectRunner DOES exercise correctly

- Single-event predicate logic (pure Python, no Beam)
- Placeholder resolution (pure Python)
- CombineFn arithmetic (percentile interpolation, distinct-set union, count sum)
- Correlation fanout graph construction (pipeline structure validity)
- Cycle detection (pure graph algorithm)
- Nested correlation alert-to-event mapping
- Type correctness (Beam's type inferencer runs on DirectRunner too)

---

## Known Limitations & Design Gaps

### 1. Pickle safety of predicate closures

**Severity:** Will crash on Dataflow if triggered.
**Scope:** All correlation transforms, not just our additions.
**Root cause:** `CompiledRule.predicate` is a closure built by `compile_rule()` in `conditions.py`. These closures capture compiled regex patterns, field name strings, and nested predicates. Regex objects ARE picklable via cloudpickle. But any future predicate that captures a non-picklable object (socket, file handle, thread lock) would break ALL correlation transforms.
**Fix:** Refactor to load rules at worker startup from GCS (the current architecture already supports this — `correlation_pipeline.py` loads rules once). The issue is that `passes_any_referenced_rule` re-evaluates predicates per-element by calling the closure, and that closure must survive the trip to the worker. The real fix is to have `SingleEventDetect` and correlation transforms receive rule YAML and compile predicates inside `setup()`.

### 2. Nested correlation group_by limitations

**Severity:** Silent wrong behavior if misconfigured.
**Root cause:** `alert_to_event()` in `correlation/nested.py` maps an Alert to a dict with only: `rule_id`, `rule_title`, `severity`, `timestamp`, `window_start`, `window_end`, `correlation_key`, `_sigma_beam_alert`. If a nested correlation rule uses `group_by: [src_ip]` (an original event field), `make_group_key` returns `"MISSING"` for every alert-event, collapsing all alerts into a single group.
**Documented behavior:** Nested correlations should use `group_by: [correlation_key]` or `group_by: [rule_id]`. A test documents this (`test_alert_to_event_supports_group_by_on_correlation_key`).
**Fix (if desired):** At load time, validate that nested correlation `group_by` fields are a subset of `{rule_id, rule_title, severity, correlation_key}` and raise `RuleLoadError` otherwise.

### 3. Mixed refs (single-event + correlation IDs in same rule)

**Severity:** Silent data loss for affected rules.
**Root cause:** If a correlation rule references both `rule-A` (single-event) and `corr-B` (another correlation), the loader classifies it as nested (because `corr-B ∈ corr_ids`). At runtime, `_build_nested_refs` only looks up `corr_by_id` — `rule-A` is in `rules_by_id` (single-event), not `corr_by_id`, so no synthetic ref is built for it. The rule only sees alert-events from `corr-B`.
**Fix options:**
  - (a) Reject mixed refs at load time with a clear error message
  - (b) Split the rule into two: one nested (refs the correlation), one first-level (refs the single-event)
  - (c) Extend `_build_nested_refs` to also consult `rules_by_id` and pass raw events alongside alert-events

### 4. Dashboard metric filters may not match

**Severity:** Dashboard tiles show empty data.
**Concern 1:** The "Alerts / min" tile filters on `resource.labels.topic_id="beaver-alerts"`. This is correct — the topic is always named `beaver-alerts` (hardcoded in `sigma_beam_io.rs`).
**Concern 2:** The "DLQ Depth" tile filters on `resource.type="pubsub_subscription"` with `subscription_id:"beaver-dlq"`. But the DLQ topic (`beaver-dlq`) has NO subscription attached by default — `sigma_beam_io.rs` only creates a subscription for the alerts topic (beaver-alerts-to-bq), not for DLQ. The DLQ is write-only. This tile needs `resource.type="pubsub_topic"` with `topic_id="beaver-dlq"` and metric `topic/send_message_operation_count` instead.
**Fix:** Change the DLQ tile in `dashboard.rs` to use topic-level metrics instead of subscription-level.

### 5. Sysmon pipeline test skipped, not verified

**Severity:** Unknown — test may pass or fail with plugin installed.
**Root cause:** `pySigma-pipeline-sysmon` is not in the venv. The test expects EventID injection, which may or may not be what the current version of the plugin does.
**Fix:** `pip install pySigma-pipeline-sysmon` into the venv, run the test, and either fix the pipeline integration or update the test expectation.

### 6. Percentile accuracy cap

**Severity:** Statistical bias under high cardinality.
**Root cause:** `MAX_SAMPLES = 50,000`. If a key receives >50k events in a window, later events are dropped from the accumulator without warning. No Beam counter or log message surfaces this.
**Fix:** Add a Beam `Counter` or `Distribution` metric that increments when samples are dropped, so operators can see it in Dataflow monitoring.

### 7. `_detect_cycles` doesn't limit nesting depth

**Severity:** Low — fanout handles >1 level correctly now but is untested.
**Root cause:** The cycle detector validates acyclicity but doesn't reject depth >2. `correlations_by_id()` now includes nested correlations, so `_build_nested_refs` can theoretically resolve refs to other nested rules. However, this was never tested end-to-end with a 3-level chain.
**Fix (if desired):** Add a test with a 3-level chain and verify it produces alerts on DirectRunner.

---

## What Would Build Confidence

Listed in priority order:

### Must-do before production deploy

1. **`beaver deploy` to a real GCP project** — verify the full pipeline: Pub/Sub → Dataflow (with sigma_beam) → BQ alerts table. This exercises pickle serialization, GCS rule loading, and real windowing in one shot.

2. **Fix DLQ dashboard tile** — change from subscription-level to topic-level metric (see limitation #4 above).

3. **Add pickle roundtrip test** — verifies that a `CompiledRule` with a real `compile_rule` predicate survives `cloudpickle.dumps` → `cloudpickle.loads`:
   ```python
   import cloudpickle, pickle
   from sigma_beam.loader import load_from_dir
   rs = load_from_dir("tests/fixtures/rules")
   for r in rs.single_event:
       restored = pickle.loads(cloudpickle.dumps(r))
       assert restored.predicate({"EventID": 4625}) == r.predicate({"EventID": 4625})
   ```

### Should-do for robustness

4. **Rewrite windowed correlation tests with `TestStream`** — gives real event-time control so we can test: late data, cross-window splits, window closure behavior. Example:
   ```python
   from apache_beam.testing.test_stream import TestStream
   ts = (TestStream()
       .add_elements([TimestampedValue(evt1, timestamp1)])
       .advance_watermark_to(timestamp1 + window_size)
       .add_elements([TimestampedValue(late_evt, timestamp1 - 1)])  # late
       ...)
   ```

5. **Install and run sysmon test** — `pip install pySigma-pipeline-sysmon` and verify.

6. **Add nested group_by validation at load time** — reject `group_by` fields that can't exist on alert-event dicts.

7. **Add `MAX_SAMPLES` exceeded counter** — Beam metric for percentile accumulator cap.

### Nice-to-have

8. **End-to-end 3-level nesting test** — verify multi-level nested correlations fire.

9. **Placeholder depth test** — write a rule with >3 levels of detection nesting and verify `_iter_detection_items` reaches all leaves.

10. **Mixed-refs decision** — decide on option (a), (b), or (c) from limitation #3 and implement.

---

## File Map (new/modified files this session)

### Beaver repo
```
.gitmodules                                    # NEW — submodule definition
.gitignore                                     # MODIFIED — removed sigma_beam/, fixed output.log entry
sigma_beam/                                    # NEW — submodule at DIvkov575/sigma-beam-backend
src/lib/dashboard.rs                           # MODIFIED — sigma_beam tiles (alerts/min, DLQ depth)
docs/superpowers/plans/2026-06-05-sigma-beam-integration.md    # Untracked plan
docs/superpowers/plans/2026-06-05-sigma-beam-review-fixes.md   # Untracked plan
docs/superpowers/plans/2026-06-05-fix-test-failures.md         # Untracked plan
```

### sigma_beam submodule
```
src/sigma_beam/placeholders.py                 # NEW — |expand resolution
src/sigma_beam/correlation/nested.py           # NEW — AlertToEvent adapter
src/sigma_beam/correlation/percentile.py       # NEW — PercentileCorrelation PTransform
src/sigma_beam/correlation/fanout.py           # MODIFIED — nested dispatch, percentile registration, warning
src/sigma_beam/correlation/temporal.py         # MODIFIED — FlatMap fix
src/sigma_beam/correlation/temporal_ordered.py # MODIFIED — FlatMap fix
src/sigma_beam/conditions.py                   # MODIFIED — safety-net, fieldref fix
src/sigma_beam/loader.py                       # MODIFIED — placeholders, two-pass, cycle detection, percentile
src/sigma_beam/ruleset.py                      # MODIFIED — nested_correlation, percentile fields
src/sigma_beam/correlation_pipeline.py         # MODIFIED — nested count in log
README.md                                      # MODIFIED — feature table, placeholder docs
tests/test_placeholders.py                     # NEW
tests/test_ruleset_nested.py                   # NEW
tests/test_loader_nested.py                    # NEW
tests/test_correlation_nested.py               # NEW
tests/test_percentile.py                       # NEW
tests/test_e2e_new_features.py                 # NEW
tests/test_pipelines.py                        # MODIFIED — skipif sysmon
tests/test_modifier_matrix.py                  # MODIFIED — xfail→passing
tests/fixtures/rules_nested/nested_correlation.yml  # MOVED from rules/
```

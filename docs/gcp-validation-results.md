# GCP Validation Results

**Date:** 2026-06-07
**Project:** `neon-circle-400322` (region `us-east1`)
**Plan:** [`docs/superpowers/plans/2026-06-05-gcp-validation.md`](superpowers/plans/2026-06-05-gcp-validation.md)
**Goal:** Validate the sigma-beam integration end-to-end on real GCP infrastructure by deploying the test pipeline (`examples/test-pipeline/beaver_config`).

This pass deployed the full pipeline against live GCP. The deploy did **not** succeed
on the first attempt — it surfaced five distinct bugs that were each diagnosed and
fixed in-place. None of these were caught by the existing unit/integration tests
because they only manifest in the real `beaver deploy` orchestration path (gcloud CLI
arg compatibility, the Dataflow template build, and venv provisioning).

## Pre-flight (passed before deploy)

- Release binary rebuilt (includes the DLQ dashboard tile fix from commit `8c7bc4f`).
- Test-pipeline ruleset loads: **5 single-event + 1 correlation** (`brute_force_5_in_60s`, `event_count`, threshold 5).
- **cloudpickle roundtrip preflight passed** — every compiled single-event predicate and the correlation rule survive `cloudpickle.dumps` → `pickle.loads`, so Dataflow's transform serialization is safe.

---

## Bugs found and fixed

### Bug 1 — `--write-metadata=false` rejected by gcloud (BQ subscription create)

**Symptom:** Deploy step `✗ sigma_beam alerts + DLQ` failed with:
```
argument --write-metadata: ignored explicit argument 'false'
```
**Root cause:** `gcloud pubsub subscriptions create --write-metadata` is a *value-less*
boolean flag; passing `=false` is invalid. (To disable you simply omit it — off is the
default.)
**Fix:** `src/lib/sigma_beam_io.rs` — removed the `--write-metadata=false` argument from
`create_alerts_subscription`. The 1:1 schema mapping is handled by `--use-table-schema`.

### Bug 2 — stale Dataflow entrypoint (`detections_template.py`)

**Symptom:** Deploy step `✗ upload Dataflow template` (template build exit 1).
**Root cause:** `examples/.../detections/detections_template.py` was still the **old,
pre-sigma_beam** pipeline. It only declared `--subscription`, called an undefined
`detections(record)`, and never imported sigma_beam — but `dataflow.rs` (correctly
updated for the integration) invokes the build with `--input_subscription`,
`--alerts_topic`, `--dlq_topic`, `--rules_uri`. argparse rejected the unknown flags.
**Fix:** Rewrote `detections_template.py` to the canonical entrypoint:
```python
from sigma_beam.correlation_pipeline import run
```
which consumes exactly the four flags the build passes.

### Bug 3 — committed broken `venv` symlink in the fixture

**Symptom:** Even with the migrated template, `source venv/bin/activate` was a no-op so
the build ran against the wrong interpreter.
**Root cause:** `examples/.../detections/venv` was **git-committed as a symlink** to an
absolute local path (`/tmp/beaver-test/detections/venv`) from the original author's
machine — a dangling link on any other host.
**Fix:** Removed the tracked symlink (`git rm --cached`) and added gitignore rules so the
deploy builds a fresh, real venv each time.

### Bug 4 — `setup_detections_venv` was dead code (never invoked)

**Symptom:** Deploy step `✗ compile sigma rules` → `detections venv setup failed`.
The venv was never created, so the template build had no apache-beam / sigma_beam.
**Root cause:** `sigma::setup_detections_venv()` existed but was **never called** in the
deploy or repair flows. The pipeline "worked" historically only because the committed
`/tmp/beaver-test` symlink (Bug 3) happened to point at a pre-existing venv on the
author's machine. Worse, the function returned `Ok(())` unconditionally, swallowing any
failure.
**Fix:**
- Wired `sigma::setup_detections_venv()` into the `compile sigma rules` step in both
  `src/commands/deploy.rs` and `src/commands/repair.rs` (before `generate_detections`,
  which depends on the venv).
- Made `setup_detections_venv` propagate a real error (and log stdout/stderr) on
  non-zero exit instead of always returning `Ok(())`.

### Bug 5 — sigma_beam editable install used a runtime env var that isn't set

**Symptom:** `detections venv setup failed (exit 1)` with the real error (initially
swallowed by the spinner UI):
```
ERROR: ./sigma_beam is not a valid editable requirement.
```
**Root cause:** `setup_detections_venv` resolved the sigma_beam path via
`std::env::var("CARGO_MANIFEST_DIR")` — a **runtime** lookup that is only populated when
running under `cargo`, **not** for the standalone release binary. It fell back to `"."`,
and because the venv script `cd`s into the detections dir before
`pip install -e <path>`, `./sigma_beam` resolved to a nonexistent relative directory.
**Fix:** `src/lib/sigma.rs` — use the compile-time `env!("CARGO_MANIFEST_DIR")` macro
(the repo root, absolute, baked in at build time) instead of the runtime `std::env::var`.
This matches the code's own stated intent and is robust to the working directory. This
was the actual blocker behind every "template build" / "venv setup" failure.

### Bug 6 — sigma_beam not shipped to Dataflow workers (runtime crash)

**Symptom:** Deploy completes and the Dataflow job launches, but workers crash-loop. The
`alerts` BigQuery table stays empty after publishing matching events. Worker logs show:
```
ModuleNotFoundError: No module named 'sigma_beam'
  ... cloudpickle.loads(s)  (bundle_processor._create_pardo_operation)
```
**Root cause:** The template build runs on the launcher, where sigma_beam is installed
(editable) in the venv. cloudpickle pickles *installed* modules **by reference**
(`import sigma_beam.x`), not by value — so the serialized graph only works where
sigma_beam is importable. The Dataflow workers run stock `apache-beam[gcp]` with **no
sigma_beam**, so unpickling every sigma_beam DoFn fails and no bundle can be processed.

> Note: the local cloudpickle roundtrip preflight passed precisely because sigma_beam
> *was* importable locally. The preflight cannot catch this class of bug — only a real
> worker (or a clean-env unpickle) can.

**Fix:** `src/lib/dataflow.rs` — before building the template, build a sigma_beam wheel
and pass it to the build with `--extra_packages`, so Dataflow stages it and each worker
pip-installs it at startup:
```sh
pip wheel "$SIGMA_BEAM_SRC" --no-deps -w "$WHEEL_DIR"
python ../artifacts/detections_gen.py ... --extra_packages "$WHEEL"
```
The sigma_beam source path is resolved with the compile-time `env!("CARGO_MANIFEST_DIR")`
(same approach as the Bug 5 fix).

### Bug 7 — alerts produced but rejected by the BigQuery subscription

**Symptom:** Pipeline runs, workers healthy, alerts are visibly published to the
`beaver-alerts` topic — but the `alerts` BigQuery table stays empty. The BQ subscription
metric `subscription/push_request_count{delivery_type=big_query}` shows a steady stream
of `response_code: invalid_argument` / `response_class: invalid` (dozens of failed writes
per sample); rows are silently dropped.
**Root cause:** The `alerts` table types `matched_events` and `tags` as **JSON** columns.
A Pub/Sub→BigQuery subscription with `use_table_schema` populates a JSON column from a
message field that is a **JSON string**, not a native nested array. sigma_beam's
`Alert.to_json()` emits those fields as native arrays, so BigQuery rejects every row with
`invalid_argument`.
**Diagnosis:** Confirmed by publishing two probe messages directly to `beaver-alerts` —
one scalars-only, one with `matched_events`/`tags` as JSON *strings*. Both landed in the
table; the native-array form did not.
**Fix:** `sigma_beam/src/sigma_beam/alerts.py` — added `Alert.to_bq_bytes()` which
JSON-encodes `matched_events` and `tags` as strings, and switched the production sink
(`_AlertToPubsubMessage` in `correlation_pipeline.py`) to use it. The general-purpose
`to_json()`/`to_bytes()` contract (and its test) is unchanged.

---

## Files changed (Rust / deploy machinery)

| File | Change |
|------|--------|
| `src/lib/sigma_beam_io.rs` | Drop invalid `--write-metadata=false` from alerts subscription create (Bug 1) |
| `src/commands/deploy.rs` | Call `setup_detections_venv` in `compile sigma rules` step (Bug 4) |
| `src/commands/repair.rs` | Same wiring for refresh-detections path (Bug 4) |
| `src/lib/sigma.rs` | `env!` instead of runtime `std::env::var` for sigma_beam path (Bug 5); fail loudly on venv setup error (Bug 4) |
| `src/lib/dataflow.rs` | Build sigma_beam wheel + `--extra_packages` so workers can import sigma_beam (Bug 6) |

## Files changed (test-pipeline fixture)

| File | Change |
|------|--------|
| `examples/test-pipeline/beaver_config/detections/detections_template.py` | Migrated to `sigma_beam.correlation_pipeline` entrypoint (Bug 2) |
| `examples/test-pipeline/beaver_config/detections/venv` | Removed committed broken symlink (Bug 3) |
| `.gitignore` | Ignore `examples/**/detections/venv` (Bug 3) |

## Files changed (sigma_beam submodule)

| File | Change |
|------|--------|
| `src/sigma_beam/alerts.py` | Add `Alert.to_bq_bytes()` (JSON-string nested fields for BQ) (Bug 7) |
| `src/sigma_beam/correlation_pipeline.py` | Production alerts sink uses `to_bq_bytes()` (Bug 7) |

### Bug 8 (observation) — failed deploys leave orphan Vector services that steal events

**Symptom:** After the pipeline was fully working, only ~2 of 7 published events reached
the detection (and the correlation never met its threshold of 5). Hop-by-hop publish
counts showed the input topic received all 8 messages but the Vector *output* topic
received only 2.
**Root cause:** The Vector input subscription has a **fixed name**
(`beaver-test-pipeline-input-sub`, from config), but each deploy creates a **randomly
suffixed** Vector Cloud Run service and a **randomly suffixed output topic**. The
repeated (initially failing) deploy attempts left four Vector services all subscribed to
the same input subscription; Pub/Sub load-balanced the events across all four, and only
the current one forwarded to the live Dataflow job's input topic. The other three
forwarded to dead orphan topics — silently dropping ~75% of events.
**Resolution for validation:** Deleted the three orphan Vector services; all subsequent
events were delivered (6/6). **Recommendation:** `beaver deploy` should roll back
partially-created resources on failure, or `destroy` should reconcile by the fixed input
subscription, so a failed/re-run deploy cannot leave competing consumers on the shared
input subscription. Not yet fixed in code — flagged here.

### Bug 9 (observation) — non-JSON ingress is dropped by Vector before the DLQ

**Symptom:** Publishing a non-JSON message to the input topic produced **no** DLQ entry,
even though the pipeline's DLQ path is correct.
**Root cause:** The Vector source is configured with `decoding.codec: json`. Vector fails
to deserialize a non-JSON frame and drops it at the ingestion stage
(`codecs::internal_events: Failed deserializing frame ... Error parsing JSON`) — it never
forwards it to the Dataflow input topic, so Dataflow's DLQ logic never sees it.
**Verification:** Publishing the same malformed bytes **directly to the Dataflow input
topic** (bypassing Vector) produced a correct `beaver-dlq` entry with the
`format_dlq` payload (`reason: non-JSON message`, error, event, traceback). So the DLQ
mechanism itself works — the gap is that truly malformed *ingress* is silently dropped by
Vector. **Recommendation:** if malformed-input visibility is desired, use Vector's
`bytes` codec (parse JSON downstream) or wire a Vector-level dead-letter sink. Not a
sigma_beam defect — flagged for awareness.

## Environment note (not a code bug)

The agent's command sandbox blocks outbound DNS to PyPI (`pypi.org` /
`files.pythonhosted.org`) while allowing GCP endpoints. The `beaver deploy` venv step
therefore must run with the sandbox disabled so `pip install apache-beam[gcp]` can
reach PyPI; GCP-only steps (gcloud / bq / Cloud Build) run fine sandboxed. This is a
property of the test harness, not of Beaver.

## Status — validated end-to-end ✅

After the seven code fixes (Bugs 1–7) plus removing the orphan Vector services (Bug 8),
the full pipeline was confirmed working against live GCP (`neon-circle-400322`):

| Check | Result |
|-------|--------|
| Deploy completes, Dataflow job reaches RUNNING | ✅ |
| Workers run without `ModuleNotFoundError` (wheel shipped) | ✅ |
| Events ingested: input topic → Vector → Dataflow | ✅ 6/6 after orphan cleanup |
| Single-event detection (`failed_login`) → BigQuery | ✅ |
| Correlation detection (`brute_force_5_in_60s`, ≥5 in 60s) → BigQuery | ✅ fired |
| Alerts persist with `matched_events`/`tags` as valid JSON | ✅ |
| DLQ catches Dataflow-stage parse errors → `beaver-dlq` | ✅ (verified by direct publish) |
| SOC dashboard + log-based metric provisioned | ✅ |

**Caveats surfaced (not sigma_beam defects):** orphan Vector services from re-run deploys
steal events on the shared input subscription (Bug 8); non-JSON ingress is dropped by
Vector's `json` codec before reaching the DLQ (Bug 9).

## Cleanup

Resources for the validation deploy plus the orphans accumulated across the (initially
failing) deploy attempts were torn down: `beaver destroy` for the live deploy, plus
manual deletion of orphan datasets (`beaver_datalake_{buib,iedo,yrar}`), buckets,
Pub/Sub topics/subscriptions, Cloud Run Vector services, service accounts, and the temp
`*-peek-tmp` debug subscriptions.

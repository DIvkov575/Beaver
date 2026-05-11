# test-pipeline

A worked example you can run by hand: five sigma rules covering different
detection patterns, a categorized set of test payloads, and an `expected.yaml`
manifest that tells you which rules each payload should fire.

There is **no orchestrator script**. The flow below is a walkthrough — run
each step yourself and observe results in the Cloud Console.

## Layout

```
beaver_config/
├── beaver_config.yaml                  # input sub: beaver-test-pipeline-input-sub
├── detections/input/                   # 5 rules
│   ├── 01_failed_login.yml             # equality on two fields
│   ├── 02_privileged_role_grant.yml    # list-membership on `role`
│   ├── 03_external_iam_grant.yml       # `selection AND NOT filter` with |endswith
│   ├── 04_suspicious_user_prefix.yml   # |startswith
│   └── 05_bulk_data_export.yml         # list-membership on `operation`
└── artifacts/                          # populated by deploy

payloads/
├── benign/        4 events that should match nothing
├── single_match/  5 events, each fires exactly one rule
├── multi_match/   2 events that fire 2-3 rules
├── edge/          5 oddballs (malformed, empty, partial, null, unicode)
└── expected.yaml  manifest: relative path → expected rule names
```

## Rule × pattern matrix

| # | Rule | Sigma feature | Severity |
|---|------|--------------|---------|
| 1 | `failed_login` | exact equality on two fields | low |
| 2 | `privileged_role_grant` | `role` is one of [owner, editor, securityAdmin] | high |
| 3 | `external_iam_grant` | `selection AND NOT filter` with `\|endswith` | medium |
| 4 | `suspicious_user_prefix` | `user \|startswith: 'temp_'` | high |
| 5 | `bulk_data_export` | `operation` is one of [export, download_bulk] | medium |

## Walkthrough

### 1. Provision the input topic

```bash
./scripts/input_topic.sh
```

Creates `beaver-test-pipeline-input` and `beaver-test-pipeline-input-sub`
(idempotent).

### 2. Deploy Beaver

```bash
BEAVER_DATAFLOW_ZONE=us-east1-d cargo run -- deploy --path examples/test-pipeline/beaver_config
```

The `BEAVER_DATAFLOW_ZONE` override dodges `us-east1-c` capacity stockouts;
remove it if your project's default zone has worker capacity.

At the end you'll see a line like:

```
Dashboard: https://console.cloud.google.com/monitoring/dashboards/builder/<id>?project=<project>
```

Open that URL — it's the SOC triage dashboard (detection feed + top rules +
rate chart + pipeline status).

### 3. Publish the test payloads

```bash
./scripts/publish_test_payloads.sh
```

This loops every file under `payloads/` and publishes its contents to the
input topic. You'll see 16 `published: ...` lines.

### 4. Watch the pipeline drain

Vector forwards messages to the output topic; the BQ subscription writes
them to `<dataset>.table1`; Dataflow runs detections and writes warnings
to Cloud Logging. Typical latency 30-90 seconds after publish.

To check BQ:

```bash
DATASET=$(grep '^[[:space:]]*dataset_id:' examples/test-pipeline/beaver_config/artifacts/resources.yaml | head -1 | awk '{print $2}')
bq query --nouse_legacy_sql --project_id=neon-circle-400322 \
  "SELECT COUNT(*) FROM \`neon-circle-400322.$DATASET.table1\`"
```

Expected: 15 rows (the 16th payload is malformed JSON and Vector drops it).

To watch detection events stream in:

- Open the dashboard URL → the "Detection events (live feed)" panel
- Or via CLI:

```bash
JOB=$(grep '^dataflow_pipeline_name:' examples/test-pipeline/beaver_config/artifacts/resources.yaml | awk '{print $2}')
JOB_ID=$(gcloud dataflow jobs list --region=us-east1 --project=neon-circle-400322 \
  --filter="name=$JOB" --format='value(JOB_ID)' | head -1)

gcloud logging read \
  "resource.type=dataflow_step AND resource.labels.job_id=$JOB_ID AND jsonPayload.message:\"BEAVER_SIEM_MATCH\"" \
  --project=neon-circle-400322 --limit=20 --format='value(jsonPayload.message)'
```

Each line is the JSON payload your detection emitted. Compare against
`payloads/expected.yaml` to see if everything matched as predicted.

### 5. Verify detection logic locally (no GCP needed)

`verify.py` replays each payload through the compiled `detections()` function
locally and compares against the manifest. Useful for iterating on rules
without redeploying.

```bash
/tmp/beaver-test/detections/venv/bin/python examples/test-pipeline/verify.py
```

Output is a per-payload PASS/FAIL.

### 6. Tear down

```bash
cargo run -- destroy --path examples/test-pipeline/beaver_config
./scripts/teardown_input.sh
```

Destroy removes everything Beaver provisioned (BQ, pubsub, Cloud Run,
Dataflow, image, repo, bucket, service accounts, dashboard, log metric).
`teardown_input.sh` removes the input topic + subscription you created in
step 1.

## Sanity check after teardown

```bash
gcloud storage buckets list --project=neon-circle-400322 --format='value(name)' | grep beaver
gcloud pubsub topics list --project=neon-circle-400322 --format='value(name)' | grep beaver
gcloud run services list --region=us-east1 --project=neon-circle-400322 --format='value(metadata.name)' | grep beaver
gcloud dataflow jobs list --region=us-east1 --project=neon-circle-400322 --status=active --format='value(name)' | grep beaver
```

All should return empty.

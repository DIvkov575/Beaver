# test-pipeline

A self-contained worked example: five sigma rules covering different
detection patterns, categorized test payloads, an `expected.yaml` manifest
that tells you which rules each payload should fire, and shell scripts
for the GCP-side bookends.

There is no orchestrator — run each step by hand and observe results in
the Cloud Console.

## Layout

```
examples/test-pipeline/
├── beaver_config/                          # the Beaver config dir
│   ├── beaver_config.yaml                  # input sub + dashboard config
│   └── detections/input/                   # 5 rules
│       ├── 01_failed_login.yml             # equality on two fields
│       ├── 02_privileged_role_grant.yml    # list-membership on `role`
│       ├── 03_external_iam_grant.yml       # `selection AND NOT filter` with |endswith
│       ├── 04_suspicious_user_prefix.yml   # |startswith
│       └── 05_bulk_data_export.yml         # list-membership on `operation`
├── payloads/                               # test inputs
│   ├── benign/                             4 events, match nothing
│   ├── single_match/                       5 events, each fires one rule
│   ├── multi_match/                        2 events, fire 2-3 rules
│   ├── edge/                               5 oddballs
│   └── expected.yaml                       relative path → expected rule names
├── scripts/                                # hand-run bookends
│   ├── input_topic.sh                      provision input pubsub
│   ├── publish_payloads.sh                 publish every payload
│   └── teardown_input.sh                   delete input pubsub
└── verify.py                               local-only detection logic check
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

All commands assume `cwd = examples/test-pipeline/`.

### 1. Provision the input pubsub topic

```bash
./scripts/input_topic.sh
```

Idempotent. Creates `beaver-test-pipeline-input` + `…-sub`.

### 2. Deploy Beaver

```bash
BEAVER_DATAFLOW_ZONE=us-east1-d cargo run --manifest-path ../../Cargo.toml -- \
    deploy --path beaver_config
```

`BEAVER_DATAFLOW_ZONE=us-east1-d` dodges `us-east1-c` capacity stockouts;
omit it if your project's default zone has worker capacity. End of deploy
prints:

```
Dashboard: https://console.cloud.google.com/monitoring/dashboards/builder/<id>?project=<project>
```

That's the SOC triage view — open it.

### 3. Publish every test payload

```bash
./scripts/publish_payloads.sh
```

16 lines, one per file under `payloads/`.

### 4. Watch the pipeline drain

Vector forwards messages to the output topic → BQ subscription writes them
to `<dataset>.table1`; Dataflow runs detections and writes warnings to
Cloud Logging. Typical latency 30-90 seconds after publish.

```bash
# Row count in BQ (expect 15 — Vector drops the malformed one)
DATASET=$(grep '^[[:space:]]*dataset_id:' beaver_config/artifacts/resources.yaml | head -1 | awk '{print $2}')
bq query --nouse_legacy_sql --project_id=neon-circle-400322 \
  "SELECT COUNT(*) FROM \`neon-circle-400322.$DATASET.table1\`"

# Detection events from the Dataflow worker
JOB=$(grep '^dataflow_pipeline_name:' beaver_config/artifacts/resources.yaml | awk '{print $2}')
JOB_ID=$(gcloud dataflow jobs list --region=us-east1 --project=neon-circle-400322 \
  --filter="name=$JOB" --format='value(JOB_ID)' | head -1)
gcloud logging read \
  "resource.type=dataflow_step AND resource.labels.job_id=$JOB_ID AND jsonPayload.message:\"BEAVER_SIEM_MATCH\"" \
  --project=neon-circle-400322 --limit=20 --format='value(jsonPayload.message)'
```

Cross-reference with `payloads/expected.yaml`.

### 5. (Optional) Verify detection logic locally

`verify.py` replays each payload through the compiled `detections()`
function and compares against `expected.yaml`. No GCP calls.

```bash
/tmp/beaver-test/detections/venv/bin/python verify.py
```

Useful when iterating on sigma rules without redeploying.

### 6. Tear down

```bash
cargo run --manifest-path ../../Cargo.toml -- destroy --path beaver_config
./scripts/teardown_input.sh
```

Destroy removes everything Beaver provisioned (BQ, pubsub, Cloud Run,
Dataflow, image, repo, bucket, service accounts, dashboard, log metric).
`teardown_input.sh` removes the input topic + sub from step 1.

### 7. Sanity check after teardown

```bash
gcloud storage buckets list --project=neon-circle-400322 --format='value(name)' | grep beaver
gcloud pubsub topics list --project=neon-circle-400322 --format='value(name)' | grep beaver
gcloud run services list --region=us-east1 --project=neon-circle-400322 --format='value(metadata.name)' | grep beaver
gcloud dataflow jobs list --region=us-east1 --project=neon-circle-400322 --status=active --format='value(name)' | grep beaver
```

All four should return empty.

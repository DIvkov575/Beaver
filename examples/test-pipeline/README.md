# test-pipeline fixture

A richer Beaver config than `examples/smoke/` — five sigma rules each
exercising a different detection pattern, plus a categorized set of test
payloads with predicted outcomes.

## Layout

```
beaver_config/
├── beaver_config.yaml                  # input subscription = beaver-test-pipeline-input-sub
├── detections/input/                   # 5 rules
│   ├── 01_failed_login.yml             # equality
│   ├── 02_privileged_role_grant.yml    # list membership
│   ├── 03_external_iam_grant.yml       # selection AND NOT filter
│   ├── 04_suspicious_user_prefix.yml   # |startswith
│   └── 05_bulk_data_export.yml         # list of values
└── artifacts/                          # populated by deploy

payloads/
├── benign/        — should pass through; nothing matches
├── single_match/  — each matches exactly one rule
├── multi_match/   — matches 2+ rules
├── edge/          — malformed, empty, partial, unicode
└── expected.yaml  — manifest mapping path → expected rule names
```

## Rule × pattern matrix

| # | Rule | Sigma feature | Severity |
|---|------|--------------|---------|
| 1 | `failed_login` | exact equality on two fields | low |
| 2 | `privileged_role_grant` | `role` is one of [owner, editor, securityAdmin] | high |
| 3 | `external_iam_grant` | `selection AND NOT filter` with `\|endswith` | medium |
| 4 | `suspicious_user_prefix` | `\|startswith: temp_` | high |
| 5 | `bulk_data_export` | `operation` is one of [export, download_bulk, query_bulk] | medium |

## Running locally (no GCP)

```
# Compile rules to detections_gen.py
cargo run -- compile --path examples/test-pipeline/beaver_config

# Inspect the merged Beam pipeline
cat examples/test-pipeline/beaver_config/artifacts/detections_gen.py
```

## Verifying detection logic against the manifest

```
# Compile rules first
cargo run -- compile --path examples/test-pipeline/beaver_config

# Replay every payload through detections() and assert against expected.yaml
/tmp/beaver-test/detections/venv/bin/python examples/test-pipeline/verify.py
```

The verifier loads each payload, runs it through the compiled `detections()`
function, captures emitted warnings, and compares the firing rule names
to `payloads/expected.yaml`.

## Running against real GCP

```
gcloud pubsub topics create beaver-test-pipeline-input
gcloud pubsub subscriptions create beaver-test-pipeline-input-sub \
  --topic=beaver-test-pipeline-input

cargo run -- deploy --path examples/test-pipeline/beaver_config

# Publish all payloads
for f in examples/test-pipeline/payloads/{benign,single_match,multi_match,edge}/*; do
  gcloud pubsub topics publish beaver-test-pipeline-input \
    --message="$(cat "$f")"
done

# Tear down
cargo run -- destroy --path examples/test-pipeline/beaver_config
gcloud pubsub subscriptions delete beaver-test-pipeline-input-sub --quiet
gcloud pubsub topics delete beaver-test-pipeline-input --quiet
```

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

## Verifying detection logic against the manifest

After a deploy populates `artifacts/detections_gen.py`:

```
/tmp/beaver-test/detections/venv/bin/python examples/test-pipeline/verify.py
```

The verifier loads each payload, runs it through the compiled `detections()`
function, captures emitted warnings, and compares the firing rule names
to `payloads/expected.yaml`.

## Running end-to-end against real GCP

```
./scripts/e2e_test_pipeline.sh
```

The script provisions the input topic, runs `cargo run -- deploy`, publishes
every payload, waits for processing, asserts Dataflow detections match
`expected.yaml`, then `cargo run -- destroy` and removes the input topic.
Trap-based cleanup runs on any exit.

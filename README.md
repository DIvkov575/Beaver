# 🦫 Beaver SIEM

**Detection-as-code SIEM on Google Cloud.** Ingest logs via Pub/Sub, run [Sigma](https://sigmahq.io) rules (single-event + correlation) on Apache Beam / Dataflow, alert into BigQuery + Pub/Sub.

Beaver is for teams that already live in GCP, want their detection rules in version control, and don't want to pay a commercial SIEM. It's not a finished product — it's a working pipeline with an explicit list of [what's missing](#whats-missing).

---

## Architecture

```
producers                  Vector                    Dataflow (sigma_beam)        sinks
─────────                  ───────                   ──────────────────────       ─────
                                           ┌────────────────────────────────┐
gcloud pubsub publish      Cloud Run        │  parse JSON ─→ DLQ topic       │
       │                   container        │      │                         │    BQ alerts table
       ▼                   running          │      ▼                         │       ▲
beaver-input ─────▶ Vector ─────▶ output ───▶  single-event rules ──────┐    │       │
   topic              │            topic    │      │                    │    │  alerts topic
                      │            │        │      ▼                    ├────┼──────▶ (severity attr)
                      │            ▼        │  correlation rules ───────┘    │       │
                      │       BigQuery      │  (event_count, value_count,    │    DLQ topic
                      │       events table  │   temporal, temporal_ordered)  │
                      │            │        │      │                         │
                      │       (hot/cold     │      ▼                         │
                      │        tiering)     │  per-rule suppression          │
                      │                     └────────────────────────────────┘
                      ▼
              parquet on GCS
              (BigLake-queryable)
```

- **Input:** any source Vector supports → an input Pub/Sub topic you provision yourself.
- **Vector** (Cloud Run, deployed by beaver): normalizes events into JSON, fans out to the output topic and writes the same events into BigQuery.
- **Dataflow streaming job** (`sigma_beam.correlation_pipeline`): loads rules from `gs://<bucket>/rules/` at worker startup, evaluates every event through single-event predicates and windowed correlations, publishes alerts.
- **Hot/cold tiering:** events land in a BigQuery table with 14-day partition expiration. A daily scheduled query rolls older partitions into `gs://<bucket>/parquet/dt=…` (ZSTD), queryable via the BigLake `events_cold` external table and the `events_all` view that unions hot + cold.

The detection engine is its own subdirectory and (eventually) its own repo: [`sigma_beam/`](sigma_beam/). See [`sigma_beam/DESIGN.md`](sigma_beam/DESIGN.md) for the runtime architecture and [`sigma_beam/README.md`](sigma_beam/README.md) for what Sigma features are supported.

---

## 5-minute quickstart

You'll need: a GCP project with billing, `gcloud` + `bq` + `gsutil` CLIs authenticated, Python 3.10–3.11 (3.12+ breaks `apache-beam` deps), and a Rust toolchain.

```bash
# 1. Clone + build
git clone https://github.com/<you>/Beaver.git && cd Beaver
./install.sh                                # rustup + python3 venv + gcloud sanity

# 2. Provision the input topic (anything you want to monitor)
gcloud pubsub topics create beaver-input --project=<project>
gcloud pubsub subscriptions create beaver-input-sub --topic=beaver-input --project=<project>

# 3. Initialize a config dir
cargo run -- init --path my-deploy
# Edit my-deploy/beaver_config.yaml — set project_id, region, input subscription
# Drop your rules under my-deploy/detections/input/

# 4. Test a rule locally before deploying
sigma-beam-test-rule my-deploy/detections/input/my-rule.yml sample-event.json

# 5. Deploy
BEAVER_DATAFLOW_ZONE=us-east1-d cargo run -- deploy --path my-deploy
# 5–10 minutes. End of deploy prints the SOC dashboard URL.

# 6. Send a synthetic event
gcloud pubsub topics publish beaver-input \
  --message='{"EventID": 4625, "User": "alice", "timestamp": "2026-05-19T00:00:00Z"}' \
  --project=<project>

# 7. Watch alerts land
bq query --nouse_legacy_sql --project_id=<project> \
  "SELECT rule_id, severity, fired_at, correlation_key FROM \`<project>.<dataset>.alerts\` 
   WHERE fired_at > TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)"

# 8. Tear down when done
cargo run -- destroy --path my-deploy
```

A polished worked example with rules, payloads, and a verification script lives at [`examples/test-pipeline/`](examples/test-pipeline/) — start there if you want to learn beaver without writing your own rules.

---

## What you can configure

Every deploy reads `<config-dir>/beaver_config.yaml`. The full schema is at [`docs/config-schema.md`](docs/config-schema.md). Minimal:

```yaml
beaver:
  project_id: my-gcp-project
  region: us-east1

sources:                            # Vector source definitions
  pubsub_in:
    type: gcp_pubsub
    project: "my-gcp-project"
    subscription: "beaver-input-sub"
    decoding: { codec: "json" }

transforms:                         # Vector transforms (optional)
  pass_through:
    type: remap
    inputs: [pubsub_in]
    source: |
      . = .
```

Optional sections:
- `dashboard:` — provision a Cloud Monitoring SOC dashboard.
- `notifications:` — email / Slack channels + alert policies.
- `billing_account:` — billing account ID for the precheck step.

Sigma rules go under `<config-dir>/detections/input/*.yml`. See [`sigma_beam/README.md`](sigma_beam/README.md) for the supported feature set.

---

## CLI

```
beaver init                 # create a config dir scaffold
beaver deploy --path X      # provision everything
beaver destroy --path X     # tear down everything (reverse order)
beaver repair-dataflow      # relaunch the Dataflow job if it died
beaver refresh-detections   # recompile rules and restart Dataflow
```

Plus the standalone tools that ship with `sigma_beam`:

```
sigma-beam-test-rule <rule.yml|dir> <event.json|jsonl|->
sigma-beam-corpus <path/to/sigma/rules>     # compile-rate report
```

---

## What works

| Capability | Status |
|---|---|
| Sigma 2.0 single-event rules | ✅ 100% SigmaHQ corpus (3132/3132 rules compile) |
| Modifiers: `endswith` / `startswith` / `contains` / `re` / `cidr` / `cased` / `exists` / `fieldref` / `windash` / `base64` / `base64offset` / `wide` / `lt` / `lte` / `gt` / `gte` / `\|all` | ✅ |
| Correlation rules: `event_count`, `value_count`, `temporal`, `temporal_ordered` | ✅ |
| Composite group-by, per-key isolation, window boundaries | ✅ |
| Per-correlation alert suppression (`beaver.suppress_window_seconds`) | ✅ |
| Threshold range (`beaver.threshold_range: [lo, hi]`) | ✅ |
| Placeholder substitution (`\|expand`) | ✅ (supply via loader) |
| Logsource filtering (block Windows rules on AWS events, etc.) | ✅ (opt-in) |
| Processing-pipeline integration (sysmon, windows, crowdstrike auto-applied) | ✅ |
| Severity-based alert routing (Pub/Sub `attributes.severity`) | ✅ |
| Dead-letter for parse + per-rule errors | ✅ |
| Hot/cold storage tiering | ✅ |
| Cloud Monitoring dashboard | ✅ |
| Notification channels (email, Slack) | ✅ |
| `destroy` symmetric to `deploy` | ✅ |
| 6386 tests (unit + integration + e2e + fuzz against SigmaHQ corpus) | ✅ |

## What's missing

Things a commercial SIEM has that beaver doesn't:

- **Analyst UI.** All alert review goes through `bq query`. No triage, no case management, no "assigned to / status" workflow.
- **Threat intel feed ingest.** No IoC-list joins.
- **User mgmt / RBAC.** Inherits GCP IAM only.
- **Backfill / replay.** Streaming-only. Historical investigation = BQ SQL by hand.
- **Multi-tenant.** One config = one deploy.
- **Onboarding wizards.** Vector source config is hand-written.
- **Hot reload of rules.** Today: `refresh-detections` restarts Dataflow. There's a sketch for in-flight rule swap via GCS-poll side input (see [`sigma_beam/DESIGN.md`](sigma_beam/DESIGN.md)) — not implemented.
- **Sigma correlation features:** `aliases` (cross-source field rename), `percentile`, `range`-as-grammar (we expose it as a `beaver.threshold_range` annotation instead), nested correlation. Rare in the SigmaHQ corpus.

Detailed gap list: [`sigma_beam/README.md`](sigma_beam/README.md#supported-sigma-2-features).

---

## Operator runbook

See [`docs/operator-runbook.md`](docs/operator-runbook.md) for: swapping rules without redeploy, rotating service accounts, draining a Dataflow job, diagnosing stuck pipelines, partial-destroy recovery.

## Alert schema + example BQ queries

See [`docs/alerts-schema.md`](docs/alerts-schema.md).

## Cold storage internals

See [`docs/efficient-storage.md`](docs/efficient-storage.md).

---

## Common issues

**`pip install` of `grpcio-tools` fails.** apache-beam supports only Python 3.8–3.11 today. Use `python3.11 -m venv` if your system Python is 3.12+.

**Dataflow worker stockouts in `us-east1-c`.** Override the zone:

```bash
BEAVER_DATAFLOW_ZONE=us-east1-d cargo run -- deploy --path …
```

**`bq` rejects table creation.** Re-auth: `gcloud auth login && gcloud auth application-default login`. The precheck step prints this fix when it detects missing ADC.

---

## License

MIT. See [`LICENSE`](LICENSE).

# `beaver_config.yaml` schema

Every deploy reads `<config-dir>/beaver_config.yaml`. This document is the complete reference for what's accepted.

A minimum working config is ~15 lines. The optional sections add a dashboard, alerting channels, and the input definition that's strictly required at runtime.

## Top-level shape

```yaml
beaver:                  # REQUIRED — project + region
  project_id: …
  region: …
  billing_account: …     # optional; only used by the precheck step

sources:                 # REQUIRED — Vector source definitions
  …

transforms:              # OPTIONAL — Vector transforms applied between sources and the output topic
  …

dashboard:               # OPTIONAL — provision a Cloud Monitoring SOC dashboard
  …

notifications:           # OPTIONAL — alerting channels and Cloud Monitoring alert policies
  …
```

## `beaver` (required)

```yaml
beaver:
  project_id: my-gcp-project       # GCP project ID; same one your gcloud config points at
  region: us-east1                 # Single region for all resources. us-east1 is well-tested.
  billing_account: 0X0X0X-…        # OPTIONAL. If present, precheck calls `gcloud billing` to link.
```

| Field | Required | Notes |
|---|---|---|
| `project_id` | yes | Must already exist; precheck doesn't create projects. |
| `region` | yes | Drives BigQuery dataset region, Dataflow region, Cloud Run region. Mixing regions is unsupported. |
| `billing_account` | no | Omitted → assume billing is already linked. |

## `sources` and `transforms` (required + optional)

These pass through verbatim into the generated Vector config (`vector.yaml`) — full reference: https://vector.dev/docs/reference/configuration/sources/. The most common shape is a single Pub/Sub source:

```yaml
sources:
  pubsub_in:
    type: gcp_pubsub
    project: "my-gcp-project"
    subscription: "beaver-input-sub"
    decoding:
      codec: "json"

transforms:
  pass_through:
    type: remap
    inputs:
      - pubsub_in
    source: |
      . = .
```

- `sources` must define at least one source.
- The first source's project + subscription are used as the *input* the Dataflow pipeline subscribes to (after Vector forwards into the output topic). If `sources` has multiple, beaver picks the lexicographically first key.
- `transforms` are optional. If omitted, Vector forwards source output directly. The example above (`. = .`) is a no-op — useful as a starting point when you want to add `parse_syslog`, `enrichment`, etc. later.

## `dashboard` (optional)

```yaml
dashboard:
  enabled: true                          # default false — section absent = no dashboard
  name: "Beaver SOC — production"        # display name in Cloud Monitoring
```

Provisions a Cloud Monitoring dashboard with tiles for: events/min, alerts/min, Pub/Sub backlog, Dataflow worker latency, DLQ depth, hot/cold tier byte counts.

## `notifications` (optional)

```yaml
notifications:
  channels:
    - name: oncall-email
      type: email
      labels:
        email_address: oncall@example.com
    - name: soc-slack
      type: slack
      labels:
        url: "https://hooks.slack.com/services/…"
  policies:
    - name: brute-force-paging
      condition_filter: |
        resource.type = "dataflow_step" AND jsonPayload.severity = "critical"
      channels: [oncall-email, soc-slack]
```

`channels[].type` may be any Cloud Monitoring channel type (`email`, `slack`, `pagerduty`, `webhook_basicauth`, etc.). `labels` are passed through to `gcloud alpha monitoring channels create`.

`policies[].channels` references channels by `name`; beaver resolves names → channel IDs after the channels are created.

## Rules

Sigma rule YAMLs go under `<config-dir>/detections/input/`. Anything matching `*.yml` or `*.yaml` is picked up. Nested subdirectories work — beaver `rglob`s the input directory.

Supported Sigma 2 surface: [`sigma_beam/README.md`](../sigma_beam/README.md#supported-sigma-2-features).

### Beaver-specific rule annotations

Add under `beaver:` in any rule's top-level YAML:

```yaml
beaver:
  suppress_window_seconds: 3600        # correlation rules only — dedupe within window
  threshold_range: [3, 5]              # correlation rules only — count must fall in [lo, hi]
  allow_high_cardinality: true         # opt out of the group-by linter
  allowed_lateness_seconds: 600        # how late events still count toward correlations
```

### Placeholders (`|expand`)

If your rules use `User|expand: '%admins%'`-style placeholders, supply a JSON file at deploy time and pass `--placeholders <file>` to `sigma-beam-test-rule` for local testing. Wiring the deploy-time placeholder upload is on the roadmap; today, bake values into your rule YAMLs or commit a placeholder config alongside.

## Artifacts written into the config dir

After `beaver deploy` runs, you'll see:

```
<config-dir>/
├── beaver_config.yaml                # what you authored
├── detections/
│   ├── input/                        # what you authored
│   ├── venv/                         # pysigma + apache-beam venv (deploy-only)
│   └── detections_template.py        # sigma_beam shim (don't edit)
└── artifacts/                        # generated; regenerated every deploy
    ├── detections_gen.py             # = detections/detections_template.py
    ├── rules/                        # staged copies of detections/input/
    ├── resources.yaml                # what beaver provisioned (used by destroy)
    └── vector.yaml                   # rendered Vector config
```

Don't commit `artifacts/`. The provided `.gitignore` excludes it.

## Schema validation

There isn't a JSON Schema / strict validator yet. The deploy step will fail with a Rust panic on a malformed config — usually with a recognizable error message. If you're seeing a parse error you can't explain, run `cargo run -- init --path /tmp/scratch` and diff against the scaffold.

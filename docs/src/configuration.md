# Configuration reference

A Beaver deploy is driven entirely by files inside `<config>/beaver_config/`:

```
beaver_config/
├── beaver_config.yaml      # top-level config (this page)
├── sigma/                  # your Sigma rule YAML files
├── notifications.yaml      # optional; alert routes (this page)
└── artifacts/              # populated by Beaver; do not edit
    ├── resources.yaml      # tracker of what got provisioned (destroy reads this)
    ├── detections_gen.py   # generated Python detection module
    └── vector.yaml         # generated Vector config
```

## `beaver_config.yaml`

```yaml
project: your-gcp-project-id        # required
region: us-east1                    # required; all resources are regional
billing_account: 0X0X0X-0X0X0X-0X0X # optional; precheck will link it if missing
service_account: optional@iam       # optional; reserved
input_subscription: my-input-sub    # required; existing Pub/Sub sub Vector reads from

dashboard:
  enabled: true                     # off by default; opt-in
  name: "My SIEM"                   # display name; gets a random suffix
```

| Field | Required | Notes |
|---|---|---|
| `project` | yes | GCP project ID. Beaver does not create the project. |
| `region` | yes | Used for Cloud Run, Dataflow, GCS, Artifact Registry. |
| `billing_account` | no | Precheck calls `gcloud beta billing projects link` if the project lacks billing. |
| `service_account` | no | Reserved for future use. |
| `input_subscription` | yes | A Pub/Sub subscription that already exists; Vector reads raw events from it. |
| `dashboard.enabled` | no | When `true`, Beaver provisions a Cloud Monitoring dashboard + log-based metric + per-component health policies. |
| `dashboard.name` | when enabled | Human-readable display name. |

## `sigma/` rules

Drop standard [Sigma YAML](https://github.com/SigmaHQ/sigma/wiki/Specification) rules into this directory. Beaver:

1. Compiles each rule via the matanolabs pySigma backend.
2. Bakes the resulting Python detection functions into `artifacts/detections_gen.py`.
3. Embeds that module into the Dataflow template at build time.

After editing a rule, run [`beaver refresh-detections`](./commands/refresh-detections.md) to recompile and relaunch the streaming job. No full redeploy needed.

## `notifications.yaml` (optional)

```yaml
channels:
  - name: soc-slack
    type: slack
    auth_token: xoxb-…              # see gcloud monitoring channel-descriptors describe slack
    channel_name: "#soc-alerts"
  - name: oncall-pager
    type: pagerduty
    service_key: …

routes:
  - match:
      severity: high
    channels: [soc-slack, oncall-pager]
  - match:
      rule_name: aws_root_account_use
    channels: [oncall-pager]
```

`channels` defines reusable notification targets. `routes` map detection events (filtered by `severity` and/or `rule_name` extracted from the harness payload) to one or more channels. Beaver:

1. Provisions one Cloud Monitoring notification channel per `channels` entry.
2. Provisions one log-based alert policy per `routes` entry.
3. Adds one `alertChart` widget per policy on the dashboard.

If `notifications.yaml` is absent or empty, the alerting section of the dashboard is omitted entirely.

## What you should not edit

- `artifacts/resources.yaml` — the tracker. [`destroy`](./commands/destroy.md) and the repair commands rely on it. If you hand-edit it, destroy may leave orphans on GCP.
- `artifacts/detections_gen.py` — regenerated on every `deploy` or `refresh-detections`.
- `artifacts/vector.yaml` — regenerated on every `deploy`.

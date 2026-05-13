# Pipeline overview

```text
                        ┌─────────────────────────────────────────────────┐
                        │                  Your GCP project                │
                        │                                                  │
  events  ─────▶ Pub/Sub │────▶ Vector ────▶ Pub/Sub ────▶ Dataflow  ──────│
  (your input)   input   │     (Cloud Run)   output       (streaming Beam) │
                  sub    │                   topic               │         │
                        │                                       │         │
                        │                                  ┌────┴────┐    │
                        │                                  │         │    │
                        │                                  ▼         ▼    │
                        │                              BigQuery   Cloud   │
                        │                              (raw + decorated   │
                        │                               events)   Logging │
                        │                                            │   │
                        │                                            ▼   │
                        │                                       Alert    │
                        │                                       policies │
                        │                                            │   │
                        │                                            ▼   │
                        │                                       Slack /  │
                        │                                       PagerDuty│
                        └─────────────────────────────────────────────────┘

                        Observability:   Cloud Monitoring dashboard with
                                         per-component health, live event feed,
                                         per-rule firing rates.
```

## Stage by stage

### 1. Ingest — Vector on Cloud Run

A [Vector](https://vector.dev/) container reads from your existing Pub/Sub input subscription (specified in `beaver_config.yaml`), applies whatever shaping you configure, and republishes to Beaver's internal output topic. Vector's config is generated at deploy time (`artifacts/vector.yaml`) and baked into the container image via Cloud Build.

Runs as a least-privilege service account (`beaver-vector-<random>`) with:
- `roles/pubsub.subscriber` on your input subscription
- `roles/pubsub.publisher` on Beaver's output topic
- `roles/logging.logWriter` project-wide

### 2. Message bus — Pub/Sub

Beaver creates a single output topic plus two subscriptions on it:
- **BigQuery subscription** (`beaver_bqsub*`) — pushes raw events directly into BigQuery via Pub/Sub's BigQuery subscription type.
- **Dataflow subscription** (`beaver_subscription_2*`) — read by the streaming job.

This fan-out means raw events land in the warehouse even if detections lag or the Dataflow job is unhealthy.

### 3. Detection — Dataflow streaming

A classic Dataflow template (built once at deploy, relaunchable any time) runs `dataflow/detections_template.py`:

1. Reads JSON events from the Dataflow subscription.
2. Passes each event through every compiled detection function in `detections_gen.py`.
3. On a match, emits a structured `BEAVER_SIEM_MATCH` log entry tagged with `rule_name`, `severity`, and the original record.

The match log entries are what every downstream alert/observability path consumes.

### 4. Notification — Cloud Logging + alert policies

For each route in `notifications.yaml`, Beaver creates a log-based alert policy whose filter matches `BEAVER_SIEM_MATCH` AND the route's match keys (severity, rule_name, etc.). When a matching event lands, the policy opens an incident and pushes to all configured notification channels.

### 5. Observability — dashboard + log-based metric

A log-based metric counts `BEAVER_SIEM_MATCH` entries with `rule_name` as a label. The dashboard uses that metric for per-rule rates and top-firing tables. See [Dashboard tour](./dashboard.md).

## Why a separate Vector tier?

You could in theory have Dataflow read directly from your input subscription, but pushing through Vector first gives you:

- A place to drop, sample, or reshape events without touching the detection code.
- Buffering and back-pressure semantics independent of the Beam worker pool.
- A clean separation between "ingest your raw event format" and "run detections" so each team owns one tier.

## Why Pub/Sub→BQ subscription instead of the Dataflow path writing to BQ?

The native Pub/Sub BigQuery subscription writes raw events with no transformation. The Dataflow path only emits *detections*, not raw events. Beaver wants both — full raw history in BQ for hunt queries, plus detections in logs for alerting — so it splits the topic into two subscriptions with two consumers.

# Dashboard tour

When `dashboard.enabled: true` in `beaver_config.yaml`, Beaver provisions a Cloud Monitoring dashboard with a uniform component-health grid plus detection-specific panels.

The dashboard URL is printed at the end of `beaver deploy` and is also recorded as `dashboard_id` in `resources.yaml`.

## Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Resources                                                                │
│ [Cloud Run] · [Dataflow] · [BigQuery] · [Output topic] · [BQ sub] · …    │
├─────────────────────────────────────────────────────────────────────────┤
│ Cloud Run    │ Dataflow      │ Output topic    │ BQ subscription        │
│ alertChart   │ alertChart    │ alertChart      │ alertChart             │
├─────────────────────────────────────────────────────────────────────────┤
│ DF sub       │ BigQuery      │ GCS bucket      │ Log metric             │
│ alertChart   │ alertChart    │ alertChart      │ alertChart             │
├─────────────────────────────────────────────────────────────────────────┤
│ Detection events (live feed) — logsPanel filtered to BEAVER_SIEM_MATCH   │
├──────────────────────────────┬──────────────────────────────────────────┤
│ Top firing rules (1h)        │ Detection rate by rule (6h)              │
│ timeSeriesTable              │ xyChart                                  │
├─────────────────────────────────────────────────────────────────────────┤
│ Dataflow worker logs (warnings + errors)                                │
├─────────────────────────────────────────────────────────────────────────┤
│ (if notifications configured)                                           │
│   Notification delivery (recent)                                        │
│   Per-route alertCharts                                                 │
└─────────────────────────────────────────────────────────────────────────┘
```

## The component-health grid

Every provisioned resource appears as an `alertChart` widget backed by a Cloud Monitoring alert policy. The widget reads green when the policy is OK and red when it's open.

| Widget | Backing policy condition |
|---|---|
| Cloud Run | no-op (instance_count > 999999) — always green if deploy succeeded |
| Dataflow | `is_failed > 0` OR `metric absent > 10min` — red on real failure or worker stockout |
| Output topic | no-op — always green |
| BQ subscription | `num_undelivered_messages > 10000` for 60s — red on real backlog |
| DF subscription | same |
| BigQuery | no-op |
| GCS bucket | no-op |
| Log metric | no-op |

**The no-op widgets are intentional.** Resources like BigQuery, GCS buckets, and idle Pub/Sub topics have no runtime metric that reliably signals "still alive". Rather than have an inconsistent grid (some scorecards, some markdown), every component gets the same alertChart UI; the ones without meaningful runtime signal show green forever as a "deploy succeeded" indicator.

The two widgets that *will* turn red on real issues are **Dataflow** and the two **subscription backlog** widgets. Watch those.

## Detection events live feed

`logsPanel` filtered to `resource.type="dataflow_step" AND jsonPayload.message:"BEAVER_SIEM_MATCH"`. New detection events stream in within a few seconds of the Dataflow harness emitting them.

## Top firing rules (1h)

`timeSeriesTable` over the log-based metric, grouped by `metric.label.rule_name`, summed over the last hour. Use this to spot which rule is generating the most volume right now.

## Detection rate by rule (6h)

`xyChart` of the same metric, rate-aligned, grouped by rule. Sustained spikes show up here before they swamp the live feed.

## Dataflow worker logs

`logsPanel` filtered to `resource.type="dataflow_step" AND resource.labels.job_name=<current> AND severity>=WARNING`. First place to look when the Dataflow alertChart goes red.

## Notification rows (optional)

When `notifications.yaml` is configured, an extra row appears:

- **Notification delivery (recent)** — `logsPanel` on `resource.type="alerting_policy"`, showing when policies fired and how the channels handled delivery.
- One **alertChart per route** — visual status of every alert policy.

## Re-rendering the dashboard

If you want to tweak the layout without a full destroy/deploy, the easiest path is to edit `src/lib/dashboard.rs::render_dashboard`, run `cargo build`, then destroy + deploy. For surgical changes against an already-running dashboard, you can also `gcloud monitoring dashboards describe` it to JSON, hand-edit, and `dashboards update`. The Rust code is the source of truth — keep them in sync.

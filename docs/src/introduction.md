# Beaver

Beaver provisions a turnkey SIEM detection pipeline on Google Cloud Platform from a single YAML config. It's a Rust CLI that wraps `gcloud` to stand up — and tear down — a real, working detection stack:

```
Pub/Sub input  →  Vector (Cloud Run)  →  Pub/Sub output  →  Dataflow (Sigma rules)  →  Pub/Sub (BQ sub)  →  BigQuery
                                                              │
                                                              └→  Cloud Monitoring (log-based metric, alert policies, dashboard)
```

You bring:
- Log events flowing into a Pub/Sub topic
- A directory of Sigma rules
- A GCP project with billing enabled

Beaver handles:
- Transport via Vector deployed on Cloud Run
- Detection pipeline compiled from Sigma to a Beam streaming job on Dataflow
- BigQuery sink for long-term storage and ad-hoc queries
- A Cloud Monitoring dashboard with per-component health alertCharts, live detection feed, top firing rules, and Dataflow worker logs
- Optional notification channels and per-route alert policies

See [Quickstart](./quickstart.md) for the fastest path to a running pipeline.

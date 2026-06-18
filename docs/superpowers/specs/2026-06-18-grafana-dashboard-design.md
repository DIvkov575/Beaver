# Grafana Dashboard Design

## Overview

Add an optional Grafana-based SIEM dashboard to Beaver as an alternative to the existing Cloud Monitoring dashboard. Deployed as a Cloud Run container (same pattern as Vector). Includes a standalone `beaver dashboard preview` command that runs a local Grafana Docker container with synthetic data for design iteration without GCP.

## Motivation

The current Cloud Monitoring dashboard is infra-centric (throughput, latency, component health). A SIEM dashboard should be security-centric — leading with detections, severity, and MITRE context — serving SOC analysts (Tier 1 triage) and security leads (posture overview).

## Design Inspirations

- **Splunk ES**: executive summary key indicators, notable events timeline, kill-chain progression
- **Elastic SIEM**: detection alerts timeline, MITRE ATT&CK rule grouping, dark theme, signal-to-case flow

---

## Dashboard Layout

### Section 1: Posture Bar (single row, always visible)

Stat/gauge tiles providing at-a-glance security posture:

| Tile | Source | Notes |
|------|--------|-------|
| Total alerts (24h) | BigQuery count | Single number, large font |
| Critical / High / Med / Low | BigQuery grouped by severity | Color-coded counters (red/orange/yellow/blue) |
| Active rules | detections_gen.py rule count | Static from deploy metadata |
| Mean time to detection | BigQuery avg(event_time - ingest_time) | Gauge, seconds |
| Pipeline health | Component health alert policies | Green/yellow/red single indicator |

### Section 2: Detection Activity (the core)

The primary working area for a Tier 1 analyst:

- **Alert timeline heatmap**: x-axis = time (5-min buckets), y-axis = severity level, color intensity = count. BigQuery time-series query. Allows visual pattern recognition of bursts.
- **Top-N firing rules table**: columns = rule name, MITRE tactic, MITRE technique, hit count (24h), last fired timestamp. Sortable, clickable (clicking filters Section 4). Data from BigQuery.
- **Source breakdown**: top 10 source IPs/hosts generating detections. Horizontal bar chart. Enables quick identification of noisy sources.

### Section 3: Pipeline Health (collapsed row, expandable)

Demoted from the current dashboard's front-and-center position:

- **Ingestion rate**: Pub/Sub push request count (same metric as current dashboard)
- **Processing latency**: Dataflow system lag
- **Dead-letter queue depth**: DLQ topic message count (non-zero = problem)
- **Component status**: row of colored dots for Vector, Dataflow, BigQuery, Pub/Sub — derived from existing component health alert policies

This section uses a Grafana collapsible row, closed by default. Analysts open it only when investigating pipeline issues.

### Section 4: Investigation Assist (drill-down)

- **Recent detections log**: table panel querying BigQuery directly — columns: timestamp, rule_name, severity, source_ip, destination_ip, raw_event (JSON expandable). Default last 1h, filterable.
- **Ad-hoc query panel**: BigQuery datasource panel where analysts can write custom SQL against the events table.
- Clicking a rule in Section 2's top-N table sets a Grafana variable that filters this section's queries.

---

## Architecture

### Config Schema

```yaml
# beaver_config.yaml
dashboard:
  type: grafana          # "cloud_monitoring" (default) | "grafana"
  name: "Beaver SIEM"
  # Grafana-specific options (ignored when type != grafana)
  grafana:
    admin_password: ""   # optional; auto-generated if empty
    allow_anonymous: false
    port: 3000           # Cloud Run port (default 3000)
```

When `dashboard.type` is omitted or `cloud_monitoring`, existing behavior is preserved unchanged.

### Deployment (Cloud Run)

Beaver builds and deploys a custom Grafana container:

1. **Base image**: `grafana/grafana-oss:latest` (or pinned version)
2. **Provisioning layer** (baked into image at deploy time):
   - `provisioning/datasources/bigquery.yaml` — BigQuery datasource configured with project ID and service account
   - `provisioning/dashboards/beaver-siem.json` — the full dashboard JSON
   - `grafana.ini` overrides — anonymous access setting, admin password, server root URL
3. **Service account**: creates a dedicated `beaver-grafana-reader` SA with BigQuery Data Viewer + Job User roles only (least privilege; separate from Dataflow SA)
4. **Cloud Run config**: port 3000, min-instances 0 (scale to zero), max-instances 1 (single dashboard viewer pattern)
5. **IAM**: the Cloud Run service is authenticated by default (only GCP project members can access). Optional `allow_anonymous: true` adds allUsers invoker binding.

### Rust Module: `src/lib/grafana/`

```
src/lib/grafana/
  mod.rs          — public API: create_grafana_dashboard(), delete_grafana_dashboard()
  config.rs       — GrafanaConfig struct, validation
  container.rs    — Dockerfile generation, Artifact Registry push
  provisioning.rs — generates datasource YAML + dashboard JSON
  dashboard.rs    — builds the Grafana dashboard JSON model (panels, rows, variables)
  preview.rs      — local Docker preview logic
```

### Dashboard JSON Generation

The dashboard JSON is built programmatically in Rust (not hand-written JSON). Structure:

```rust
pub struct GrafanaDashboard {
    pub title: String,
    pub panels: Vec<Panel>,
    pub templating: Vec<Variable>,
    pub time: TimeRange,
}

pub struct Panel {
    pub title: String,
    pub panel_type: PanelType,  // stat, table, heatmap, timeseries, row, logs
    pub grid_pos: GridPos,
    pub targets: Vec<Target>,   // BigQuery SQL or metric queries
    pub options: serde_json::Value,
}
```

Panels reference Grafana variables (`$severity`, `$rule_name`, `$timeFilter`) for cross-filtering.

---

## CLI Commands

### `beaver dashboard preview`

Runs a local Grafana container with synthetic data for offline preview:

1. Generates the dashboard JSON from config
2. Generates a SQLite datasource provisioning file loaded with synthetic events
3. Writes a temporary Dockerfile extending `grafana/grafana-oss` with provisioning files mounted
4. Runs `docker run --rm -p 3000:3000` (foreground, Ctrl-C to stop)
5. Opens `http://localhost:3000` in the default browser (or prints URL)

Does not require GCP credentials, a deployed pipeline, or any running infrastructure.

### `beaver dashboard export`

Writes the Grafana dashboard JSON to `artifacts/grafana-dashboard.json`. Importable into any existing Grafana instance manually.

### `beaver deploy --path <dir>` (existing, extended)

When `dashboard.type: grafana`, the deploy sequence adds steps:
- Build Grafana container image
- Push to Artifact Registry
- Deploy Cloud Run service
- Record `grafana_service_url` in resources.yaml

### `beaver destroy --path <dir>` (existing, extended)

Deletes the Grafana Cloud Run service and Artifact Registry image if present.

---

## Synthetic Data (Preview Mode)

A bundled dataset at `src/lib/grafana/fixtures/synthetic_events.json`:

- ~500 sample detection events spanning 24 hours
- 8-10 distinct rule names covering different MITRE tactics (Initial Access, Execution, Persistence, Lateral Movement, Exfiltration)
- Severity distribution: ~5% critical, ~15% high, ~40% medium, ~40% low
- Varied source/destination IPs, hostnames
- Realistic timestamps with clustering (simulated attack bursts)

At preview time, this is loaded into a SQLite database that Grafana's SQLite datasource plugin queries. The dashboard JSON uses SQL queries compatible with both the SQLite preview datasource and the production BigQuery datasource (via Grafana's datasource variable `${DS_BEAVER}`).

---

## Datasource Abstraction

Dashboard panels use a Grafana datasource variable (`${DS_BEAVER}`) so the same dashboard JSON works against:
- **Production**: BigQuery datasource (real events)
- **Preview**: SQLite datasource (synthetic events)

SQL queries are written in a common subset (standard SQL) that both backends support. Where BigQuery-specific syntax is needed (e.g., `TIMESTAMP_DIFF`), the query uses Grafana conditionals or the preview datasource provides compatible UDFs.

---

## Theme and Styling

- Dark theme (default Grafana dark, matches Elastic SIEM aesthetic)
- Severity color palette: Critical=#FF4444, High=#FF8800, Medium=#FFCC00, Low=#4488FF
- Dashboard uses Grafana's built-in responsive grid (24-column layout)
- Section 1 (posture bar): full width, height 3 units
- Section 2 (detection activity): full width, height 10 units
- Section 3 (pipeline health): collapsible row, height 6 units when open
- Section 4 (investigation): full width, height 8 units

---

## Testing Strategy

- **Unit tests** (Rust): dashboard JSON generation produces valid JSON, correct panel structure, correct query templates, variables resolve
- **Integration test**: `beaver dashboard preview` starts container, HTTP 200 on `/api/health`, dashboard loads via API
- **Snapshot test**: generated dashboard JSON is deterministic given same config inputs (golden file comparison)

---

## Scope Boundaries

**In scope:**
- Grafana Cloud Run deployment as alternative dashboard type
- Dashboard JSON generation (4 sections as described)
- Preview command with synthetic data
- Export command
- Config schema extension

**Out of scope (future work):**
- Alerting via Grafana (continue using Cloud Monitoring alert policies)
- User authentication beyond GCP IAM (no LDAP/SAML integration)
- Multiple dashboard pages (single dashboard for now)
- Grafana plugins beyond BigQuery datasource and SQLite (for preview)
- Case management / incident workflow

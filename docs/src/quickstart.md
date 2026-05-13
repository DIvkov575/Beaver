# Quickstart

## Prerequisites

- A GCP project with billing enabled
- `gcloud` and `bq` installed, authenticated (`gcloud auth login` + `gcloud auth application-default login`)
- A Rust toolchain (`cargo` 1.70+) to build Beaver itself

## Install

Pick whichever fits.

### One-line installer (macOS / Linux)

```bash
curl -sSfL https://raw.githubusercontent.com/Divkov575/Beaver/main/install.sh | sh
```

Detects your OS/arch, downloads the matching release binary from GitHub, and drops it at `~/.local/bin/beaver`. The script prints a PATH hint if that dir isn't already on `$PATH`. Override the install dir with `BEAVER_INSTALL_DIR=/usr/local/bin`, or pin a version with `BEAVER_VERSION=v0.3.0`.

### Cargo

```bash
cargo install beaver-siem
```

Installs as `beaver` on `~/.cargo/bin` (which most Rust users already have on PATH).

### From source

```bash
git clone https://github.com/Divkov575/Beaver
cd Beaver
cargo build --release
# binary at target/release/beaver
```

## Scaffold a config

```bash
beaver init --path my-beaver --dev
```

This creates `my-beaver/beaver_config/` containing:

- `beaver_config.yaml` — top-level config (project, region, dashboard, notifications)
- `sigma/` — your Sigma rule YAMLs go here
- `artifacts/` — generated files (Python, vector.yaml, resources.yaml) Beaver writes during deploy

Edit `beaver_config.yaml` to point at your project, region, and an existing Pub/Sub input subscription.

## Deploy

```bash
beaver deploy --path my-beaver/beaver_config
```

Beaver runs a sequence of named steps: precheck, sigma compile, BQ, Pub/Sub, GCS, Vector SA, Vector image build, Cloud Run, Dataflow SA, Dataflow template, Dataflow job, optional notifications, dashboard. Each step appears as a spinner with success/failure.

Total runtime: 6–10 minutes on a fresh project.

## Send a test event

If you used the bundled `examples/test-pipeline`:

```bash
cd examples/test-pipeline
./scripts/input_topic.sh         # creates the harness input topic/sub
./scripts/publish_payloads.sh    # publishes the test payloads
```

You should see detection events in the dashboard's live feed within ~30s, and in BigQuery within ~1 min.

## Iterate

Edit rules and re-deploy just the detection logic:

```bash
beaver refresh-detections --path my-beaver/beaver_config
```

This recompiles Sigma, re-uploads the Dataflow template, and relaunches the job — no other infra is touched.

## Tear down

```bash
beaver destroy --path my-beaver/beaver_config
```

`destroy` is a plan-then-execute pass over `resources.yaml`. Anything Beaver provisioned gets deleted; pre-existing project resources are untouched.

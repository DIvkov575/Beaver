# Commands

Beaver exposes five subcommands. All take `--path <dir>` pointing at your `beaver_config/` directory.

| Command | Purpose | Affects GCP |
|---|---|---|
| [`init`](./init.md) | Scaffold a new `beaver_config/` skeleton. | No |
| [`deploy`](./deploy.md) | Provision everything from scratch. | Yes — creates ~15 resources |
| [`destroy`](./destroy.md) | Tear down everything tracked in `resources.yaml`. | Yes — deletes everything Beaver created |
| [`repair-dataflow`](./repair-dataflow.md) | Relaunch only the Dataflow job. Idempotent if it's already Running. | Yes — Dataflow job only |
| [`refresh-detections`](./refresh-detections.md) | Recompile Sigma rules and relaunch Dataflow with the new code. | Yes — Dataflow job only |

## Mental model

The system has two layers of state:

- **Slow-moving infra**: SAs, IAM grants, BQ, Pub/Sub, GCS, Cloud Run, Artifact Registry, dashboards. Created once at `deploy`, destroyed by `destroy`. Rarely needs to change.
- **Fast-moving compute**: the Dataflow job + the detection code it embeds. Crashes, runs out of workers, gets new rules — needs to be replaced often without touching the rest.

That split is why `repair-dataflow` and `refresh-detections` exist as separate commands: most operational needs only touch the fast layer, so you shouldn't have to destroy and redeploy ~15 resources just to swap out a streaming job.

## Recommended workflows

| Situation | Command |
|---|---|
| First-time setup | `init` then `deploy` |
| Edited a Sigma rule | `refresh-detections` |
| Dataflow job failed (zone exhaustion, OOM, crash) | `repair-dataflow` |
| Changed `beaver_config.yaml` or `vector.yaml` shape | `destroy` then `deploy` (no targeted repair yet) |
| Done with the deploy | `destroy` |

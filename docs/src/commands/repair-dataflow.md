# repair-dataflow

Relaunch only the Dataflow job. Use when the streaming job has failed (zone exhaustion, OOM, crash) but the rest of the pipeline is healthy.

```bash
beaver repair-dataflow --path <config>
```

## What it does

1. Loads `resources.yaml` and looks up the recorded `dataflow_pipeline_name`.
2. Calls `gcloud dataflow jobs list` to read the current `currentState`.
3. If `Running` → exits as no-op.
4. Otherwise:
   - Best-effort cancels the existing job (returns OK if already inactive).
   - Re-uploads the Dataflow template to GCS.
   - Launches a new streaming job with a fresh random name.
   - Polls `currentState` until `Running` (up to 5 min) or surfaces recent worker errors on failure.
5. Updates `dataflow_pipeline_name` in `resources.yaml` to the new name.

Everything else — SAs, IAM grants, GCS bucket, Cloud Run service, BQ, Pub/Sub, dashboard — is **not touched**. The repair assumes those are still healthy.

## When to use

| Symptom | Use this? |
|---|---|
| Dataflow job is FAILED or CANCELLED, rest of pipeline looks fine | ✅ |
| Dataflow alertChart on dashboard is red | ✅ |
| Cloud Run is also broken | ❌ — use `destroy` + `deploy` |
| You edited rules, not just the job state | use [`refresh-detections`](./refresh-detections.md) instead |
| Workers exhausted in default zone | ✅ — set `BEAVER_DATAFLOW_ZONE` then run repair |

## Common pattern: zone exhaustion

The default Dataflow zone (`<region>-d`) is intermittently stockout-prone for `n1-standard-2` workers. If you see `ZONE_RESOURCE_POOL_EXHAUSTED` in worker logs:

```bash
BEAVER_DATAFLOW_ZONE=us-east1-b beaver repair-dataflow --path <config>
```

The new job lands in the override zone. You can keep that env var set for subsequent repairs until the default zone recovers.

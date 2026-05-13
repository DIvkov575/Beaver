# refresh-detections

Recompile Sigma rules and relaunch Dataflow with the new detection code.

```bash
beaver refresh-detections --path <config>
```

## What it does

1. Loads `resources.yaml`.
2. Runs the sigma compile step → regenerates `artifacts/detections_gen.py`.
3. Cancels the current Dataflow job **unconditionally** (even if Running — it's running stale code).
4. Re-uploads the Dataflow template with the freshly compiled module baked in.
5. Launches a new streaming job (fresh random name).
6. Polls `currentState` until `Running`.

Everything else stays put.

## When to use

| Situation | Use this |
|---|---|
| Added a new Sigma rule | ✅ |
| Edited an existing rule's logic | ✅ |
| Renamed a rule | ✅ |
| Deleted a rule | ✅ |
| Job is failing for a *non*-code reason (zone exhaustion, IAM) | use [`repair-dataflow`](./repair-dataflow.md) — it's the same flow without the recompile |

## Cost of using this

Because step 3 cancels the running job unconditionally, you'll get a ~2–3 min gap where the detection pipeline is down (workers spin up, template loads). Raw events still land in BigQuery via the BQ subscription throughout — only the detections layer is interrupted.

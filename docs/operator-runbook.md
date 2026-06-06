# Operator runbook

Common operational tasks. Assumes `<config-dir>` is the directory you deployed against and `<project>` is the GCP project ID.

## Swap or update rules

Two paths depending on what you're changing:

**Single rule change, no schema impact.** Drop the new YAML into `<config-dir>/detections/input/`, then:

```bash
cargo run -- refresh-detections --path <config-dir>
```

This recompiles the rule set, re-uploads it to `gs://<bucket>/rules/`, and restarts the Dataflow job. Downtime ≈ 30–90 seconds.

**Schema-impact change** (renaming the alerts table, adding new Pub/Sub topics, etc.): a full `destroy` → `deploy` cycle. Always verify `resources.yaml` is empty after destroy before redeploying.

**Test a rule before pushing.** Run it against a sample event locally — no GCP roundtrip:

```bash
sigma-beam-test-rule path/to/my-rule.yml sample-event.json
sigma-beam-test-rule detections/input/ events.jsonl
```

## Stop accepting events but keep alerts queryable

The simplest "pause" is to delete the input Pub/Sub subscription. Beaver doesn't manage the input topic (you created it pre-deploy), so deleting the subscription stops the flow without touching anything beaver provisioned:

```bash
gcloud pubsub subscriptions delete beaver-input-sub --project=<project>
```

Recreate when ready. Beaver's Vector instance will keep itself alive — it just has nothing to read.

For longer pauses (≥1 day), `cargo run -- destroy --path <config-dir>` is cleaner. The alerts BQ table is dropped along with the dataset, so export anything you need first.

## Repair a dead Dataflow job

If the job died but the rest of beaver is still running:

```bash
cargo run -- repair-dataflow --path <config-dir>
```

This relaunches the job using the existing template + the rules that were last uploaded. Doesn't re-upload rules. Downtime ≈ 30–60 seconds.

Diagnose first if you don't know why it died:

```bash
JOB=$(grep '^dataflow_pipeline_name:' <config-dir>/artifacts/resources.yaml | awk '{print $2}')
JOB_ID=$(gcloud dataflow jobs list --region=<region> --project=<project> \
    --filter="name=$JOB" --format='value(JOB_ID)' | head -1)
gcloud logging read \
    "resource.type=dataflow_step AND resource.labels.job_id=$JOB_ID AND severity>=ERROR" \
    --project=<project> --limit=30 --format=json | jq -r '.[].textPayload'
```

Common causes: worker OOM (raise `--worker-machine-type` in `dataflow.rs`), Pub/Sub quota (request more), worker zone stockout (set `BEAVER_DATAFLOW_ZONE=<region>-d` and repair).

## Diagnose alerts not landing

Walk the pipeline from output back to input:

1. **Are there *any* events in the events table?**
   ```bash
   bq query --nouse_legacy_sql --project_id=<project> \
     "SELECT COUNT(*) FROM \`<project>.<dataset>.table1\` 
      WHERE _PARTITIONTIME > TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)"
   ```
   `0` means Vector isn't forwarding. Check Cloud Run logs:
   ```bash
   gcloud run services logs read beaver-vector-<suffix> --region=<region> --project=<project> --limit=50
   ```

2. **Are events reaching the output topic?**
   ```bash
   gcloud pubsub subscriptions pull <output-sub-id> --auto-ack --limit=5 --project=<project>
   ```

3. **Is Dataflow RUNNING?**
   ```bash
   gcloud dataflow jobs list --region=<region> --project=<project> --status=active
   ```
   If `JOB_STATE_FAILED`, see "Repair a dead Dataflow job" above.

4. **Are alerts being produced?** Watch the alerts topic directly:
   ```bash
   gcloud pubsub subscriptions create alerts-tail --topic=beaver-alerts --project=<project>
   gcloud pubsub subscriptions pull alerts-tail --auto-ack --limit=5 --project=<project>
   gcloud pubsub subscriptions delete alerts-tail --project=<project>
   ```

5. **Are alerts being written to BQ?** If the topic has messages but the table doesn't:
   ```bash
   gcloud pubsub subscriptions describe beaver-alerts-to-bq-<suffix> --project=<project> \
       --format='value(deadLetterPolicy,bigqueryConfig)'
   ```
   Check Pub/Sub's "dropped messages" metric in Cloud Monitoring for the subscription.

6. **Is a rule actually matching anything?** The Dataflow job emits per-rule counters as `custom.googleapis.com/sigma_beam/matches_<rule_id>`. If a rule shows 0, test it locally:
   ```bash
   sigma-beam-test-rule detections/input/my-rule.yml sample-event.json -v
   ```

## DLQ inspection

Pull from the DLQ topic to see what's failing:

```bash
gcloud pubsub subscriptions create dlq-tail --topic=beaver-dlq --project=<project>
gcloud pubsub subscriptions pull dlq-tail --auto-ack --limit=20 --project=<project> --format=json \
    | jq -r '.[].message.data' | base64 -d | jq .
gcloud pubsub subscriptions delete dlq-tail --project=<project>
```

`reason: "non-JSON message"` means Vector forwarded something the JSON parser couldn't read — check your `transforms`. `reason: "predicate raised"` means a Sigma rule's compiled predicate threw — the entry includes `rule_id`, the event, and the exception traceback. Reproduce locally with `sigma-beam-test-rule`.

## Rotate the Dataflow service account

The service account is the only IAM principal that publishes to the alerts/DLQ topics and writes to BigQuery, so rotation requires coordination:

```bash
# 1. Find current SA from resources.yaml
SA=$(grep '^dataflow_sa_email:' <config-dir>/artifacts/resources.yaml | awk '{print $2}')

# 2. Create a key (only if you actually need a key — workload identity prefers no keys)
gcloud iam service-accounts keys create new-key.json --iam-account=$SA --project=<project>

# 3. Restart Dataflow so workers pick up the new credentials
cargo run -- repair-dataflow --path <config-dir>

# 4. Delete the old key (after confirming workers came up healthy)
gcloud iam service-accounts keys list --iam-account=$SA --project=<project>
gcloud iam service-accounts keys delete <old-key-id> --iam-account=$SA --project=<project>
```

To swap the SA entirely: destroy + redeploy. The SA email is baked into the Dataflow job's worker_service_account at launch and can't be changed without relaunching.

## Drain a Dataflow job for migration

Beaver's `repair-dataflow` cancels and relaunches — losing in-flight events. To drain without loss (Dataflow finishes processing what's already in-flight before stopping):

```bash
JOB=$(grep '^dataflow_pipeline_name:' <config-dir>/artifacts/resources.yaml | awk '{print $2}')
JOB_ID=$(gcloud dataflow jobs list --region=<region> --project=<project> \
    --filter="name=$JOB" --format='value(JOB_ID)' | head -1)
gcloud dataflow jobs drain $JOB_ID --region=<region> --project=<project>

# Wait for state = JOB_STATE_DRAINED, then:
cargo run -- repair-dataflow --path <config-dir>
```

## Partial-destroy recovery

If `destroy` fails partway, `resources.yaml` retains the entries that still exist. Re-run `destroy` — it's idempotent. If a single resource is stuck (e.g., a manually-modified subscription that destroy can't delete), delete it manually with `gcloud` and remove its entry from `resources.yaml` so the next `destroy` doesn't trip over it.

## Backup the alerts table before destroy

Beaver's `destroy` drops the BQ dataset (and the alerts table with it). Export first if you want history:

```bash
DATASET=$(grep '^[[:space:]]*dataset_id:' <config-dir>/artifacts/resources.yaml | head -1 | awk '{print $2}')
bq extract \
    --destination_format=NEWLINE_DELIMITED_JSON \
    --compression=GZIP \
    "<project>:$DATASET.alerts" \
    "gs://<your-backup-bucket>/alerts-$(date +%Y%m%d)/*.jsonl.gz"
```

The cold-tier Parquet at `gs://<deploy-bucket>/parquet/dt=…` isn't auto-deleted by `destroy` (bucket-recursive delete handles it, but only after lifecycle ages out). Move to a backup bucket first if you need the history.

## Common gotchas

| Symptom | Likely cause | Fix |
|---|---|---|
| `gcloud dataflow jobs run` fails immediately | Worker zone stockout | `BEAVER_DATAFLOW_ZONE=<region>-d cargo run -- deploy …` |
| `pip install` of grpcio-tools fails during `init` | Python 3.12+ | Use `python3.11 -m venv` |
| Alerts topic exists but BQ table empty | Pub/Sub service agent missing `bigquery.dataEditor` | Re-run deploy — beaver's first step grants it; or `gcloud projects add-iam-policy-binding` manually |
| `destroy` leaves Pub/Sub subscriptions behind | Manually-modified subscription | Delete by hand, remove from `resources.yaml` |
| `refresh-detections` doesn't pick up a new rule | Rule has a YAML parse error | Run `sigma-beam-test-rule` against it locally first |
| Correlation rule doesn't fire | Event-time field missing/malformed | Default expected field is `timestamp` (ISO 8601). Check `parse_event_time` in `correlation/_common.py` |
| `bq` rejects `mk --time_partitioning_field=fired_at` | Already exists | Idempotent — destroy first, or ignore the "already exists" warning |

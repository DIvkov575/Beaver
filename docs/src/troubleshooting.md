# Troubleshooting

Real failures encountered while building and operating Beaver.

## Dataflow workers never come up: `ZONE_RESOURCE_POOL_EXHAUSTED`

**Symptom**: `beaver deploy` hangs at "wait for Dataflow workers to come up", then errors out. Dataflow console shows the job in `JOB_STATE_FAILED` with `ZONE_RESOURCE_POOL_EXHAUSTED` errors.

**Cause**: GCP's auto-zone selector picked a zone that's out of `n1-standard-2` capacity. Common in `us-east1-c` and `us-east1-d`.

**Fix**: pin a different zone and relaunch just Dataflow:

```bash
BEAVER_DATAFLOW_ZONE=us-east1-b beaver repair-dataflow --path <config>
```

The default deploy also accepts this env var if you know up front:

```bash
BEAVER_DATAFLOW_ZONE=us-east1-b beaver deploy --path <config>
```

## "SA does not exist" on IAM grant after SA create succeeded

**Symptom**: `beaver deploy` creates a service account, then fails on the immediately-following `add-iam-policy-binding` with `Service account beaver-…@… does not exist`.

**Cause**: IAM consistency lag — newly-created SAs are not visible to IAM bindings for ~10–20 seconds.

**Fix**: Beaver already handles this. `create_sa` polls `describe` for up to 10s after create, and every grant is wrapped in `run_iam_grant_with_retry` (5s × 6 attempts). If you still hit it, GCP is unusually laggy — re-running `deploy` should converge.

## Dataflow worker writes failing: bucket access denied

**Symptom**: Worker logs show `Permission denied on bucket dataflow-staging-<region>-<projectnum>`.

**Cause**: The Dataflow SA only has `objectAdmin` on Beaver's own bucket; the auto-managed staging bucket needs a separate grant.

**Fix**: Beaver grants both at deploy time (`grant_bucket(dataflow_staging, ..., "roles/storage.objectAdmin")`). If you see this after a successful deploy, the grant was applied — re-run `repair-dataflow` to relaunch with a fresh template upload.

## Dashboard alertChart shows "no data available"

**Symptom**: A component-health widget on the dashboard shows "No data available for the selected time frame" instead of red/green.

**Cause**: The backing metric isn't emitting any data points at all. For Dataflow this happens when workers never started — `is_failed` and friends only emit while there are workers. For idle Cloud Run / Pub/Sub topics, the metrics are emitted only when there's traffic.

**Fix**: For Dataflow, the dashboard policy already has a `conditionAbsent` clause that flips red after 10 min of no data. Wait the duration, or relaunch via `repair-dataflow` to get real metric data flowing.

## `gcloud monitoring channels create` says command not found

**Symptom**: Notification channel provisioning fails with "unknown command".

**Cause**: The command lives under `gcloud beta monitoring channels`, not `gcloud monitoring channels`.

**Fix**: Beaver invokes the beta-prefixed form already. If you see this on your own, install/update `gcloud beta`:

```bash
gcloud components install beta
```

## `gcloud alpha monitoring policies` says "components not installed"

**Fix**: `gcloud components install alpha`. Beaver's precheck runs this automatically (`ensure_alpha_components`).

## `beaver destroy` panics with "No such file or directory"

**Symptom**: `Tracker.save()` panics during destroy.

**Cause**: Older `resources.yaml` files stored `config_path` as a relative path. If you run destroy from a different cwd than the original deploy, save fails.

**Fix**: Resolved as of the current build. `destroy` and the repair commands now rewrite `config_path` to the absolute form of the `--path` arg on load. If you hit this on an old build, hand-edit `resources.yaml` and replace `config_path:` with the absolute path.

## Pub/Sub→BQ subscription delivery failing

**Symptom**: Events flow into Vector and the output topic, but BigQuery stays empty. Dashboard `BQ writes/s` (if you still have backlog widgets) shows nothing.

**Cause**: The Pub/Sub service agent (`service-<projnum>@gcp-sa-pubsub.iam.gserviceaccount.com`) lacks `roles/bigquery.dataEditor`.

**Fix**: Beaver's deploy step "grant Pub/Sub→BQ delivery" handles this. If you removed it manually, re-grant:

```bash
gcloud projects add-iam-policy-binding PROJECT_ID \
  --member=serviceAccount:service-PROJECT_NUMBER@gcp-sa-pubsub.iam.gserviceaccount.com \
  --role=roles/bigquery.dataEditor
```

## Cloud Build keeps using a managed bucket I can't access

**Symptom**: Vector image build fails with `permission denied on gs://PROJECT_ID_cloudbuild`.

**Cause**: Cloud Build's default GCS bucket needs Cloud Build SA permissions on it. Usually self-fixes after the first build; sometimes new projects need a one-shot enable.

**Fix**:

```bash
gcloud services enable cloudbuild.googleapis.com --project=PROJECT_ID
```

Beaver's precheck enables this API automatically.

## Resources lingering after destroy

**Symptom**: After `beaver destroy`, you still see Beaver-prefixed alert policies, log metrics, etc. in the GCP console.

**Cause**: Anything created *outside* `resources.yaml` is invisible to destroy. Most common case: alert policies created by hand via `gcloud` while iterating on dashboard design.

**Fix**: Clean up manually:

```bash
gcloud alpha monitoring policies list --filter='displayName:Beaver' --format='value(name)' \
  | xargs -I{} gcloud alpha monitoring policies delete {} --quiet

gcloud logging metrics list --filter='name~beaver_' --format='value(name)' \
  | xargs -I{} gcloud logging metrics delete {} --quiet
```

# destroy

Tear down everything tracked in `artifacts/resources.yaml`.

```bash
beaver destroy --path <config>
```

## What it does

`destroy` is a **plan-then-execute** pass over `resources.yaml`. It walks the tracker in reverse dependency order and calls the appropriate `delete_*` for each:

```
Dashboard
LogMetric
AlertPolicy (one per route + one per component health check)
NotificationChannel
DataflowJob          (cancels via JOB_ID lookup)
CrsService
PubsubBqSubscription
PubsubSubscription2
PubsubTopic
BqDataset
ArtifactImage
ArtifactRepo
GcsBucket            (force-empties on delete)
VectorSa
DataflowSa
```

Every individual delete is idempotent — `NOT_FOUND`, `does not exist`, and similar errors are treated as success — so re-running `destroy` after a partial run is safe and converges.

## What `destroy` does *not* touch

- Your input Pub/Sub topic/subscription (Beaver only consumed it)
- GCP-managed buckets like `dataflow-staging-<region>-<projnum>` and `<project>_cloudbuild`
- Any resource created outside Beaver and not present in `resources.yaml`

If you've poked at `resources.yaml` by hand, or provisioned policies/channels via `gcloud` directly (the way the dashboard iteration phase sometimes does), `destroy` will not see those and you'll need to delete them manually.

## After successful destroy

If every planned step succeeds, `resources.yaml` is removed:

```text
destroy complete; resources.yaml removed
```

If anything failed, `resources.yaml` keeps the entries that didn't get cleaned up, so a re-run can pick up where it left off:

```text
destroy partial; resources.yaml retains failed entries
```

## Verifying

Spot-check what's left:

```bash
gcloud dataflow jobs list --region=<region> --status=active
gcloud run services list --region=<region>
gcloud pubsub topics list | grep beaver_
gcloud iam service-accounts list --filter='email:beaver-*'
gcloud alpha monitoring policies list --filter='displayName:Beaver'
gcloud logging metrics list --filter='name~beaver_'
```

# deploy

Provision a full Beaver pipeline from a `beaver_config/` directory.

```bash
beaver deploy --path <config>
```

## What it does

Runs the following steps in order, each as a spinner. Every step writes back to `artifacts/resources.yaml` as soon as the resource is created, so a crash mid-deploy leaves an accurate record that `destroy` can later clean up.

1. **environment precheck** — checks `gcloud`/`bq` CLIs, ADC, the active project, optionally links billing, enables 9 required APIs, and installs `gcloud alpha` components.
2. **compile sigma rules** — runs pySigma over `sigma/` and generates `artifacts/detections_gen.py`.
3. **grant Pub/Sub→BQ delivery** — gives the Pub/Sub service agent `roles/bigquery.dataEditor` so BQ subscriptions can deliver.
4. **BigQuery dataset + table** — creates `beaver_<rand>` dataset and the events table.
5. **Pub/Sub topic + subscriptions** — output topic plus the BQ subscription and the Dataflow pull subscription.
6. **GCS bucket** — `beaver_<rand>` for Dataflow staging + temp.
7. **Vector SA + IAM** — creates `beaver-vector-<rand>@`, grants resource-scoped subscriber/publisher and `logging.logWriter`.
8. **write vector.yaml** — renders the Vector config that points at input sub → output topic.
9. **build Vector docker image (Cloud Build)** — submits a Cloud Build job that bakes vector.yaml into the image.
10. **deploy Vector Cloud Run service** — runs the image as `beaver-vector-instance-<rand>`.
11. **Dataflow SA + IAM** — creates `beaver-dataflow-<rand>@`, grants Dataflow subscriber, objectAdmin on the Beaver bucket *and* on `dataflow-staging-<region>-<projectnum>`, plus `dataflow.worker` and `logging.logWriter`.
12. **upload Dataflow template** — builds the classic template from `detections_gen.py` and uploads to `gs://<bucket>/templates/`.
13. **launch Dataflow streaming job** — runs the template as `beaver-detections-<rand>`. Pins worker zone to `<region>-d` by default to avoid known-stockout zones. Override with `BEAVER_DATAFLOW_ZONE`.
14. **wait for Dataflow workers to come up (up to 5 min)** — polls `currentState` until `Running`. On terminal failure, pulls recent worker errors from Cloud Logging into the error message.
15. **notification channels + alert policies** — *only if* `notifications.yaml` is present.
16. **SOC dashboard + log-based metric** — *only if* `dashboard.enabled: true`. Creates the log-based metric, 8 per-component health alert policies, and the dashboard itself.

## Idempotency

`deploy` is not currently fully idempotent. Re-running it against an existing `resources.yaml` will fail on resource collisions (BQ dataset exists, topic exists, etc.). Use [`destroy`](./destroy.md) followed by `deploy` to start over, or use the targeted [`repair-dataflow`](./repair-dataflow.md) / [`refresh-detections`](./refresh-detections.md) commands for the common partial-failure cases.

## Environment overrides

| Variable | Effect |
|---|---|
| `BEAVER_DATAFLOW_ZONE` | Pin Dataflow workers to a specific zone (e.g. `us-east1-b`). Useful when the default zone is stockout-prone. |

## Output

On success Beaver prints a summary block plus a dashboard URL (if enabled):

```text
Deployed:
  BigQuery dataset    my-project.beaver_a1b2c3.events
  Pub/Sub output      beaver_x9y8z7
  GCS bucket          beaver_h7i8j9
  Vector (Cloud Run)  beaver-vector-instance-d4e5f6
  Dataflow job        beaver-detections-s1t2u3
  Vector SA           beaver-vector-d4e5f6@my-project.iam.gserviceaccount.com
  Dataflow SA         beaver-dataflow-d4e5f6@my-project.iam.gserviceaccount.com

Dashboard: https://console.cloud.google.com/monitoring/dashboards/builder/abc-123?project=my-project
```

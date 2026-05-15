# Efficient Storage (Hot/Cold Tiering)

## What changed

Beaver now tiers BigQuery storage automatically. The schema is unchanged — still one `data: JSON` column.

- **Hot:** existing BQ table, now day-partitioned by ingestion time, 14-day partition expiration, `require_partition_filter=true`.
- **Cold:** zstd-Parquet on GCS under `gs://<bucket>/parquet/dt=YYYY-MM-DD/`, exposed as BigLake external table `events_cold`.
- **Unified surface:** view `events_all` = hot `UNION ALL` cold, with a normalized `partition_date` column. Cold rows are STRING-encoded JSON re-parsed via `SAFE.PARSE_JSON` for analyst ergonomics.
- **Rollover:** BigQuery scheduled query runs daily — exports the 13-day-old partition to Parquet, then `DELETE`s it from the hot table.
- **GCS lifecycle:** Standard → Nearline @30d → Coldline @90d → Archive @365d.

## Why

BigQuery active storage is ~$0.02/GB-month; the same data on Coldline is ~$0.004 and on Archive ~$0.0012. Most SIEM queries hit the last ~2 weeks, so we keep that fast/clustered/native in BQ and move the long tail to GCS where it costs ~5–15× less.

## Components added (single module: `src/lib/cold_storage.rs`)

| Component | What it is |
|---|---|
| BigLake connection | `CLOUD_RESOURCE` connection in the deploy region; its managed SA gets read on the bucket. |
| Sentinel parquet | One 0-row file written at deploy time so the external table has a schema to bind to before the first scheduled run. |
| External table | `CREATE EXTERNAL TABLE … WITH PARTITION COLUMNS WITH CONNECTION` over `gs://…/parquet/*`, `metadata_cache_mode=AUTOMATIC`, `max_staleness=INTERVAL '1' HOUR`. |
| View `events_all` | Hot+cold UNION with `partition_date` column. |
| Scheduled query | `bq mk --transfer_config` running daily — `EXPORT DATA OPTIONS(format='PARQUET', compression='ZSTD', overwrite=true)` then `DELETE`. Wrapped in `EXECUTE IMMEDIATE FORMAT(...)` so the partition date interpolates into the URI. |
| Export SA | Dedicated `beaver-export-*` SA the scheduled query runs as: `bigquery.jobUser`, `bigquery.dataEditor`, `storage.objectAdmin` on the bucket. Deploy-time invoker gets `serviceAccountTokenCreator` on it. |

## Tunable constants (top of `cold_storage.rs`)

- `HOT_RETENTION_DAYS = 14`
- `EXPORT_AGE_DAYS = 13` (must be `<` retention)
- `EXPORT_SCHEDULE = "every 24 hours"`
- `PARQUET_PREFIX = "parquet"`

## Notes on the implementation

- **JSON columns can't be exported to Parquet.** Sentinel + scheduled export both `TO_JSON_STRING(data)`; cold-side view does `SAFE.PARSE_JSON(data)`.
- **Dataset is now regional** (`bq mk --dataset --location=<region>`) so BigLake (regional) can read it.
- **BigLake SA IAM lag**: the `add-iam-policy-binding` for the connection SA retries up to 8× with linear backoff — newly created SAs aren't visible to IAM for ~30–60s.
- **External table creation uses DDL**, not `--external_table_definition=@file` (latter was rejected as "Invalid source URI").
- **Late data** arriving more than 13 days after `_PARTITIONDATE` will fall between hot expiration and cold export. Mitigation if it matters: shorten `EXPORT_AGE_DAYS` or partition on `event_time` instead of ingest time (requires schema change — out of scope here).

## Deploy + destroy

Deploy gains one new step right after the GCS bucket step. Destroy walks the four new resource types (scheduled query, view, cold table, BigLake connection) + the export SA before the dataset.

## Out of scope (deferred)

- Typed/normalized columns (`source`, `event_type`, `host`, …). Adding these would require a producer-side normalization layer (Vector VRL or transformer) and is its own plan.
- Clustering. Only meaningful once real columns exist.

## Verification

48 unit tests (argument shape + SQL/DDL contents) + 1 `#[ignore]`d real-GCP smoke that creates the whole cold tier and tears it down in ~52s.

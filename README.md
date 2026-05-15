# 🦫 Beaver SIEM

*Secure. Analyze. Monitor.*

Beaver SIEM is a cutting-edge **data security log analysis tool** designed to protect your infrastructure by monitoring and analyzing logs in real-time. It ensures your system’s safety by helping identify potential security breaches and threats through advanced log parsing and analysis techniques.

> “Stay secure, stay informed.”


### **Common Issues **
- Initialization failed 
  ```        
  ERROR: Failed building wheel for grpcio-tools
      Failed to build grpcio-tools
      ERROR: Failed to build installable wheels for some pyproject.toml based projects (grpcio-tools)
      [end of output]
  
  note: This error originates from a subprocess, and is likely not a problem with pip.
  error: subprocess-exited-with-error
  ```
  Source: Apache beam has limited python 3.x support
  Solution: Check currently supported python versions (3.8, 3.9, 3.10, 3.11 as of 4/22/25)


[//]: # (### **enabling apis**)
[//]: # (gcloud services enable cloudscheduler.googleapis.com run.googleapis.com)


## Storage tiers

Beaver writes events into a **hot** BigQuery table (`<dataset>.<table>`,
day-partitioned by ingestion time with a 14-day partition expiration).
Daily, a BigQuery scheduled query rolls 13-day-old partitions out to
`gs://<bucket>/parquet/dt=YYYY-MM-DD/*.parquet` (zstd, Hive-partitioned)
and deletes them from the hot table. The cold prefix is exposed through a
BigLake external table (`<dataset>.events_cold`); the view
`<dataset>.events_all` unions hot + cold and exposes a unified `partition_date`
column.

GCS lifecycle: Standard → Nearline (30d) → Coldline (90d) → Archive (365d).

Tune retention/export age in `src/lib/cold_storage.rs` (constants at top).
The schema is unchanged: a single `data: JSON` column on both tiers.

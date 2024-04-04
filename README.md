Beaver SIEM (data security log analysis tool)
----
**Todo:**
- destructured log -> bq
- batching -> deduplication + writes
- logging

**Ideas**
- disabling detections_gen.py regeneration
- create input pubsub (to route log sink into)

**left off on**

---
create bq
create pubsub topic (1) + subscription (2) (bq) (general)
create bucket

create crj -> bucket & pubsub 
create df -> bucket & pubsub

---

[gcp log sink logs not appearing in pubsub](https://stackoverflow.com/questions/68778305/gcp-log-router-sink-not-routing-logs-to-topic)
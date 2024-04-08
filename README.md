Beaver SIEM (data security log analysis tool)
----
**Todo:**

**l_0**
- crj shutoff
- missing '{}' as second arg in last .get() chain method call

**l_1**
- add sa delegation
- destructured log -> bq
- logging



**Ideas**
- disabling detections_gen.py regeneration
- batching -> deduplication + writes (after log destructuring) (increase bq write efficiency)

**left off on**
create crs

---
create bq
create pubsub topic (1) + subscription (2) (bq) (general)
create bucket

create crj -> bucket & pubsub 
create df -> bucket & pubsub

---

[gcp log sink logs not appearing in pubsub](https://stackoverflow.com/questions/68778305/gcp-log-router-sink-not-routing-logs-to-topic)
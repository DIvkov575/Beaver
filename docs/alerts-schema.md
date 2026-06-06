# Alerts: schema, attributes, and example queries

Every alert beaver fires lands in two places:

1. **`<project>.<dataset>.alerts`** — BigQuery table, day-partitioned on `fired_at`, 30-day expiration. Pub/Sub subscription `beaver-alerts-to-bq-*` pushes alerts here.
2. **`projects/<project>/topics/beaver-alerts`** — Pub/Sub topic. Each message has `severity` and `rule_id` attributes for native subscriber-side filtering.

Both carry the same payload.

## BigQuery table schema

| Column | Type | Notes |
|---|---|---|
| `rule_id` | STRING | UUID from the Sigma rule's `id:` field. |
| `rule_title` | STRING | `title:` from the rule. |
| `severity` | STRING | `level:` from the rule. One of `informational` / `low` / `medium` / `high` / `critical`. Defaults to `medium` when the rule omits `level`. |
| `fired_at` | TIMESTAMP | Wall-clock time the alert was emitted. Partition column. |
| `window_start` | TIMESTAMP | Correlation window start. NULL for single-event matches. |
| `window_end` | TIMESTAMP | Correlation window end. NULL for single-event matches. |
| `correlation_key` | STRING | `\|`-joined group-by values. NULL for single-event matches. For composite groups like `(User, SourceIP)`, the key reads `alice\|10.0.0.1`. |
| `matched_events` | JSON | For single-event rules: one entry, the offending event (or just the rule's `fields:` projection if set). For correlation rules: an empty array (events not retained to keep state cheap). |
| `tags` | JSON | Array of dotted-namespace tags from the Sigma rule's `tags:` field. Example: `["attack.t1059", "attack.execution", "cve.2024-1234"]`. |

## Pub/Sub message attributes

| Attribute | Value |
|---|---|
| `severity` | Same as the BQ column. Filter critical-only with `attributes.severity = "critical"`. |
| `rule_id` | Same as the BQ column. Useful for per-rule routing without payload parsing. |

Subscriber filter example (route critical alerts to PagerDuty):

```bash
gcloud pubsub subscriptions create beaver-alerts-pagerduty \
  --topic=beaver-alerts \
  --message-filter='attributes.severity = "critical"' \
  --push-endpoint=https://events.pagerduty.com/integration/.../enqueue \
  --project=<project>
```

## Example queries

### Top 10 firing rules in the last hour

```sql
SELECT rule_id, rule_title, COUNT(*) AS hits
FROM `<project>.<dataset>.alerts`
WHERE fired_at > TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 1 HOUR)
GROUP BY rule_id, rule_title
ORDER BY hits DESC
LIMIT 10
```

### Critical alerts since midnight

```sql
SELECT fired_at, rule_id, rule_title, correlation_key, tags
FROM `<project>.<dataset>.alerts`
WHERE severity = 'critical'
  AND fired_at > TIMESTAMP_TRUNC(CURRENT_TIMESTAMP(), DAY)
ORDER BY fired_at DESC
```

### Brute-force correlation keys with the most fires

```sql
SELECT correlation_key, COUNT(*) AS fires,
       MIN(fired_at) AS first_seen, MAX(fired_at) AS last_seen
FROM `<project>.<dataset>.alerts`
WHERE rule_id = '<your-brute-force-rule-uuid>'
  AND fired_at > TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 7 DAY)
GROUP BY correlation_key
ORDER BY fires DESC
LIMIT 25
```

### Alerts with a specific MITRE technique

```sql
SELECT fired_at, rule_id, rule_title, severity
FROM `<project>.<dataset>.alerts`
WHERE 'attack.t1110' IN UNNEST(JSON_VALUE_ARRAY(tags))
ORDER BY fired_at DESC
LIMIT 50
```

### Pull the offending event payload for a single-event alert

```sql
SELECT rule_id, rule_title, JSON_QUERY(matched_events, '$[0]') AS event
FROM `<project>.<dataset>.alerts`
WHERE rule_id = '<single-event-rule-uuid>'
  AND fired_at > TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 1 HOUR)
ORDER BY fired_at DESC
```

### Per-severity rate over the last 24h

```sql
SELECT TIMESTAMP_TRUNC(fired_at, HOUR) AS hour, severity, COUNT(*) AS n
FROM `<project>.<dataset>.alerts`
WHERE fired_at > TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 24 HOUR)
GROUP BY hour, severity
ORDER BY hour DESC, severity
```

### Correlate alerts with the underlying events

`<dataset>.events_all` is a view that unions hot + cold event tiers (see [`docs/efficient-storage.md`](efficient-storage.md)). Join it against `alerts` on correlation key:

```sql
SELECT a.rule_id, a.fired_at, a.correlation_key,
       JSON_VALUE(e.data, '$.User') AS user,
       JSON_VALUE(e.data, '$.EventID') AS event_id
FROM `<project>.<dataset>.alerts` a
JOIN `<project>.<dataset>.events_all` e
  ON JSON_VALUE(e.data, '$.User') = a.correlation_key
 AND e.partition_date BETWEEN DATE(a.window_start) AND DATE(a.window_end)
WHERE a.rule_id = '<brute-force-uuid>'
  AND a.fired_at > TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 7 DAY)
LIMIT 100
```

## Where to read alerts in real time

| Sink | When you want it |
|---|---|
| `bq query` / scheduled BQ exports | Reporting, triage, dashboards |
| Pub/Sub push subscription | Routing to PagerDuty, OpsGenie, Slack via webhook |
| Pub/Sub pull subscription | Custom analyst tooling, SIEM connectors |
| Dataflow → Pub/Sub → another Beam job | Second-tier correlation, ML scoring |

## DLQ schema

Parse errors and per-rule predicate errors land on `projects/<project>/topics/beaver-dlq` as JSON:

```json
{
  "reason": "predicate raised",
  "rule_id": "11111111-1111-1111-1111-111111111111",
  "event": { "EventID": 4625, "...": "..." },
  "error": "TypeError: unsupported operand type(s) for +: 'int' and 'NoneType'",
  "trace": "Traceback (most recent call last):..."
}
```

`reason` is one of:
- `non-JSON message` — parse failure at ingestion
- `predicate raised` — a Sigma rule's compiled predicate threw an exception on this event

No automatic BQ subscription is provisioned for the DLQ — it's intentionally low-volume and operator-pull. Subscribe with a pull subscription and inspect when investigating malformed events.

# Writing Sigma rules

Beaver runs standard [Sigma](https://github.com/SigmaHQ/sigma) YAML through the [`pySigma-backend-matano`](https://github.com/matanolabs/pySigma-backend-matano) backend, which compiles each rule into a Python function. Those functions get bundled into the Dataflow streaming job at deploy time.

## Rule layout

Rules live in `beaver_config/sigma/`. Beaver picks up `*.yaml` recursively. A minimal rule:

```yaml
title: Suspicious sudo from non-admin
id: 1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d
description: A non-admin user invoked sudo.
status: experimental
author: you
date: 2026/01/01
logsource:
  product: linux
  service: syslog
detection:
  selection:
    program: sudo
  filter:
    user|startswith: "admin-"
  condition: selection and not filter
fields:
  - user
  - program
  - command
level: high
tags:
  - attack.privilege_escalation
```

## How matching works at runtime

For every event the Dataflow harness receives from the output Pub/Sub subscription:

1. The harness parses it as JSON.
2. Calls every compiled detection function with the parsed dict.
3. On any function returning truthy, emits a structured log entry:
   ```json
   {
     "event": "BEAVER_SIEM_MATCH",
     "rule_name": "Suspicious sudo from non-admin",
     "severity": "high",
     "record": { ... original event ... },
     "message": "Suspicious sudo from non-admin"
   }
   ```
4. That log entry flows to Cloud Logging, which:
   - Increments the log-based metric (per `rule_name` label) — drives dashboard panels
   - Triggers any alert policies whose filter matches

## What fields can rules use?

Whatever's in your event JSON after Vector finishes shaping it. The Sigma backend translates conditions to Python attribute access on a dict. So if Vector emits:

```json
{ "user": "alice", "program": "sudo", "command": "/bin/bash" }
```

then `selection: { program: sudo }` translates to `record.get("program") == "sudo"`.

## Severity → alert routing

Beaver extracts `level:` from the rule, normalizes it to the `severity` label, and writes it into every match event. Your `notifications.yaml` routes can then match on it:

```yaml
routes:
  - match: { severity: critical }
    channels: [oncall-pager]
  - match: { severity: high }
    channels: [soc-slack]
```

See [Configuration reference](./configuration.md) for the full notification schema.

## Iterating on rules

1. Edit rule YAMLs in `beaver_config/sigma/`.
2. Run `beaver refresh-detections --path beaver_config`. This recompiles and relaunches the Dataflow job only — no other infra is touched.
3. Publish a test event and watch the dashboard's live feed.

If the compile step fails, the spinner shows `✗ compile sigma rules` and the pySigma error is included in the output. Fix the rule and re-run.

## Limitations

- Only the matanolabs pySigma backend is wired in. Rules that depend on Splunk-specific functions or ES query DSL will not compile.
- No correlation rules yet (each event is evaluated independently).
- Rule changes require a Dataflow job restart (~2–3 min gap) since the rules are compiled into the streaming code rather than loaded dynamically.

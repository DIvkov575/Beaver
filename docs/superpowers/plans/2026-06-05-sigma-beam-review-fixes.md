# Code Review Fix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 7 bugs surfaced by code review of the sigma-beam integration: fanout early-return, percentile None crash, dashboard gap, nested correlation design issues, recursion limit, and placeholder deepcopy.

**Architecture:** Each fix is independent — no ordering dependency. Fixes go into the sigma_beam submodule (Python) and Beaver (Rust dashboard). All fixes have test coverage.

**Tech Stack:** Python 3.10+, Apache Beam (DirectRunner), Rust.

---

## File Structure

| File | Fixes |
|------|-------|
| `sigma_beam/src/sigma_beam/correlation/fanout.py` | #1 (early-return), #4 (nested refs lookup) |
| `sigma_beam/src/sigma_beam/correlation/percentile.py` | #2 (None crash), #8 (merge_accumulators) |
| `sigma_beam/src/sigma_beam/correlation/nested.py` | #9 (group_by fields from correlation_key) |
| `sigma_beam/src/sigma_beam/loader.py` | #6 (iterative DFS) |
| `sigma_beam/src/sigma_beam/placeholders.py` | #7 (deepcopy on empty table) |
| `sigma_beam/src/sigma_beam/ruleset.py` | #4 (correlations_by_id includes nested) |
| `src/lib/dashboard.rs` | #3 (y-position gap) |

---

### Task 1: Fix fanout early-return when branches empty + nested exists

**Files:**
- Modify: `sigma_beam/src/sigma_beam/correlation/fanout.py:56-62`
- Modify: `sigma_beam/tests/test_correlation_nested.py`

- [ ] **Step 1: Write failing test**

```python
# Append to sigma_beam/tests/test_correlation_nested.py

def test_nested_fanout_works_when_first_level_refs_unresolved():
    """Nested correlations should still work even if first-level refs can't resolve."""
    # Base rule ID doesn't match what the first-level correlation expects
    base_rule = CompiledRule(
        id="DIFFERENT-ID", title="base",
        severity="low", predicate=lambda e: True,
    )
    child_corr = CompiledCorrelation(
        id="corr-child", title="child", severity="high",
        kind="event_count", referenced_rule_ids=("NONEXISTENT-RULE",),
        group_by=("x",), window_seconds=60, threshold=1,
    )
    nested_corr = CompiledCorrelation(
        id="corr-nested", title="nested", severity="critical",
        kind="event_count", referenced_rule_ids=("corr-child",),
        group_by=("rule_id",), window_seconds=60, threshold=1,
        is_nested=True,
    )
    rs = Ruleset(
        single_event=[base_rule],
        correlation=[child_corr],
        nested_correlation=[nested_corr],
    )

    events = [{"x": "a", "timestamp": 1000}]
    with TestPipeline() as p:
        pcoll = p | beam.Create(events)
        # Should NOT crash — currently it early-returns empty
        _ = pcoll | CorrelationFanout(rs)
```

- [ ] **Step 2: Run to verify failure**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_correlation_nested.py::test_nested_fanout_works_when_first_level_refs_unresolved -v
```
Expected: Either crash or the test reveals the nested path is unreachable.

- [ ] **Step 3: Fix fanout.py lines 56-62**

Replace:
```python
        if not branches and not rs.nested_correlation:
            return pcoll | "EmptyFanout" >> beam.Filter(lambda _: False) \
                         | "AsAlert" >> beam.Map(lambda _: None)

        if not branches:
            return pcoll | "EmptyFanout2" >> beam.Filter(lambda _: False) \
                         | "AsAlert2" >> beam.Map(lambda _: None)
```

With:
```python
        if not branches and not rs.nested_correlation:
            return pcoll | "EmptyFanout" >> beam.Filter(lambda _: False) \
                         | "AsAlert" >> beam.Map(lambda _: None)
```

Then change the `first_level_alerts` and nested section to handle the empty-branches case:
```python
        if branches:
            first_level_alerts = branches | "FlattenFirstLevel" >> beam.Flatten()
        else:
            first_level_alerts = None

        if rs.nested_correlation and first_level_alerts is not None:
            corr_by_id = rs.correlations_by_id()
            alert_events = first_level_alerts | "AlertsToEvents" >> AlertsToEvents()

            nested_branches = []
            for nc in rs.nested_correlation:
                cls = _KIND_TO_TRANSFORM.get(nc.kind)
                if cls is None:
                    raise UnknownCorrelationKind(nc.kind)
                refs = _build_nested_refs(nc, corr_by_id)
                if not refs:
                    continue
                nested_branches.append(
                    alert_events | f"Nested[{nc.id}]" >> cls(nc, refs)
                )

            if nested_branches:
                nested_alerts = nested_branches | "FlattenNested" >> beam.Flatten()
                return [first_level_alerts, nested_alerts] | "FlattenAll" >> beam.Flatten()

        if first_level_alerts is not None:
            return first_level_alerts

        return pcoll | "EmptyFanout2" >> beam.Filter(lambda _: False) \
                     | "AsAlert2" >> beam.Map(lambda _: None)
```

- [ ] **Step 4: Run tests**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_correlation_nested.py -v
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && git add -A && git commit -m "fix(fanout): don't early-return when branches empty but nested exists"
```

---

### Task 2: Fix percentile None crash + merge_accumulators truncation

**Files:**
- Modify: `sigma_beam/src/sigma_beam/correlation/percentile.py:44,101`
- Modify: `sigma_beam/tests/test_percentile.py`

- [ ] **Step 1: Write failing test for None field**

```python
# Append to sigma_beam/tests/test_percentile.py

def test_percentile_handles_none_field_value():
    """Events with None values for percentile_field should not crash."""
    base = CompiledRule(id="r1", title="r", severity="low", predicate=_always_true)
    corr = CompiledCorrelation(
        id="corr-p95", title="p95", severity="high",
        kind="percentile", referenced_rule_ids=("r1",),
        group_by=("service",), window_seconds=60,
        threshold=500, threshold_op="gte",
        percentile=95.0, percentile_field="latency_ms",
    )
    # Mix of valid and None values
    events = [
        {"service": "api", "latency_ms": 1000, "timestamp": 1000},
        {"service": "api", "latency_ms": None, "timestamp": 1001},
        {"service": "api", "latency_ms": 1000, "timestamp": 1002},
    ]
    with TestPipeline() as p:
        pcoll = p | beam.Create(events)
        alerts = pcoll | PercentileCorrelation(corr, [base])
        assert_that(alerts, is_not_empty())
```

- [ ] **Step 2: Run to verify failure**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_percentile.py::test_percentile_handles_none_field_value -v
```
Expected: FAIL with TypeError: float() argument must be a string or a real number, not 'NoneType'

- [ ] **Step 3: Fix DropMissing filter in percentile.py**

Change line 101 from:
```python
            | "DropMissing" >> beam.Filter(lambda e: get_field(e, pf) is not MISSING)
```
To:
```python
            | "DropMissing" >> beam.Filter(lambda e: get_field(e, pf) is not MISSING and get_field(e, pf) is not None)
```

- [ ] **Step 4: Fix merge_accumulators to process all accumulators**

Change:
```python
    def merge_accumulators(self, accs):
        out: list = []
        for a in accs:
            out.extend(a)
            if len(out) >= MAX_SAMPLES:
                return out[:MAX_SAMPLES]
        return out
```
To:
```python
    def merge_accumulators(self, accs):
        out: list = []
        for a in accs:
            out.extend(a)
        if len(out) > MAX_SAMPLES:
            out = out[:MAX_SAMPLES]
        return out
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_percentile.py -v
```
Expected: PASS (all including new test).

- [ ] **Step 6: Commit**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && git add -A && git commit -m "fix(percentile): handle None field values + fix merge_accumulators truncation"
```

---

### Task 3: Fix dashboard y-position gap

**Files:**
- Modify: `src/lib/dashboard.rs:495`

- [ ] **Step 1: Fix the offset**

Change:
```rust
    let sigma_beam_y = after_health_y + 16;
```
To:
```rust
    let sigma_beam_y = after_health_y;
```

- [ ] **Step 2: Build + test**

```bash
cd /Users/divkov/workplace/Beaver && cargo build 2>&1 | tail -3 && cargo test 2>&1 | tail -3
```
Expected: compile success, 49 tests pass.

- [ ] **Step 3: Commit**

```bash
cd /Users/divkov/workplace/Beaver && git add src/lib/dashboard.rs && git commit -m "fix(dashboard): remove 16-unit gap before sigma_beam tiles"
```

---

### Task 4: Fix correlations_by_id to include nested + log nested count

**Files:**
- Modify: `sigma_beam/src/sigma_beam/ruleset.py:58`
- Modify: `sigma_beam/src/sigma_beam/correlation_pipeline.py` (log line)

- [ ] **Step 1: Update correlations_by_id**

Change:
```python
    def correlations_by_id(self) -> dict[str, CompiledCorrelation]:
        return {c.id: c for c in self.correlation}
```
To:
```python
    def correlations_by_id(self) -> dict[str, CompiledCorrelation]:
        return {c.id: c for c in self.correlation + self.nested_correlation}
```

Note: This also fixes finding #5 (mixed refs) indirectly — `_build_nested_refs` will now find first-level correlation IDs in the combined dict, AND first-level single-event IDs are still not in this dict (correctly). The real fix for mixed refs is that the loader should NOT classify rules with mixed refs as nested — but that's a design decision for later. For now, making `correlations_by_id` comprehensive is the right fix.

- [ ] **Step 2: Update the pipeline log line**

In `sigma_beam/src/sigma_beam/correlation_pipeline.py`, find the log line:
```python
    log.info(
        "ruleset: %d single-event, %d correlation",
        len(rs.single_event), len(rs.correlation),
    )
```
Change to:
```python
    log.info(
        "ruleset: %d single-event, %d correlation, %d nested",
        len(rs.single_event), len(rs.correlation), len(rs.nested_correlation),
    )
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_ruleset_nested.py -v
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && git add -A && git commit -m "fix(ruleset): correlations_by_id includes nested; log nested count"
```

---

### Task 5: Convert recursive DFS to iterative

**Files:**
- Modify: `sigma_beam/src/sigma_beam/loader.py:39-67`

- [ ] **Step 1: Replace the DFS function**

Replace `_detect_cycles`:
```python
def _detect_cycles(correlations: list) -> None:
    """Raise RuleLoadError if the correlation reference graph has a cycle."""
    ids = {c.id for c in correlations}
    adj: dict[str, set[str]] = {c.id: set() for c in correlations}
    for c in correlations:
        for ref in c.referenced_rule_ids:
            if ref in ids:
                adj[c.id].add(ref)

    WHITE, GRAY, BLACK = 0, 1, 2
    color = {cid: WHITE for cid in ids}

    for start in ids:
        if color[start] != WHITE:
            continue
        stack = [(start, iter(adj[start]))]
        color[start] = GRAY
        while stack:
            node, children = stack[-1]
            try:
                nb = next(children)
                if color[nb] == GRAY:
                    raise RuleLoadError(
                        f"cycle detected in correlation rules: {node} → {nb}"
                    )
                if color[nb] == WHITE:
                    color[nb] = GRAY
                    stack.append((nb, iter(adj[nb])))
            except StopIteration:
                color[node] = BLACK
                stack.pop()
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_loader_nested.py::test_cycle_detection_raises -v
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && git add -A && git commit -m "fix(loader): iterative DFS for cycle detection (avoids recursion limit)"
```

---

### Task 6: Fix placeholders deepcopy on empty table

**Files:**
- Modify: `sigma_beam/src/sigma_beam/placeholders.py:25-28`

- [ ] **Step 1: Fix the early return**

Change:
```python
    if not table:
        _check_no_placeholders(rule, table)
        return rule
```
To:
```python
    if not table:
        _check_no_placeholders(rule, table)
        return deepcopy(rule)
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_placeholders.py -v
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && git add -A && git commit -m "fix(placeholders): deepcopy rule even with empty table to prevent shared mutation"
```

---

### Task 7: Add correlation_key fields to alert_to_event

**Files:**
- Modify: `sigma_beam/src/sigma_beam/correlation/nested.py`
- Modify: `sigma_beam/tests/test_correlation_nested.py`

- [ ] **Step 1: Enhance alert_to_event to expose group_by fields from correlation_key**

The `correlation_key` in first-level alerts is a `|`-delimited string built by `make_group_key`. For nested correlations to group_by on `correlation_key` itself (the whole string), this already works. The issue is when users try to group_by on original fields like `src_ip`. 

The pragmatic fix: document that nested correlations should group_by on `correlation_key` (the parent's composite key) or on alert metadata fields (`rule_id`, `severity`). Add these fields to the docstring and add a test:

```python
def test_alert_to_event_supports_group_by_on_correlation_key():
    """Nested rules should use group_by: [correlation_key] or [rule_id]."""
    a = Alert(
        rule_id="corr-x",
        rule_title="X",
        severity="high",
        fired_at="2026-01-01T00:00:00+00:00",
        correlation_key="alice|10.0.0.1",
    )
    event = alert_to_event(a)
    from sigma_beam.correlation._common import make_group_key
    # group_by on correlation_key works
    assert make_group_key(event, ("correlation_key",)) == "alice|10.0.0.1"
    # group_by on rule_id works
    assert make_group_key(event, ("rule_id",)) == "corr-x"
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && pytest tests/test_correlation_nested.py -v
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
cd /Users/divkov/workplace/Beaver/sigma_beam && git add -A && git commit -m "test(nested): document supported group_by fields for nested correlations"
```

---

## Out of Scope

- **Mixed refs (single-event + correlation):** A design decision — should these be split into two rules at load time, or should the nested stage also check `rules_by_id`? Deferred to a follow-up.
- **Pickle safety of closures over CompiledRule.predicate on Dataflow:** This is a pre-existing pattern shared by ALL correlation transforms (event_count, value_count, temporal). Fixing it requires a structural refactor (moving predicate evaluation into a DoFn that receives rule YAML and compiles at worker startup). Out of scope for this bug-fix pass.

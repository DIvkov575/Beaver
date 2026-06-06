# sigma-beam Full Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin sigma-beam-backend as a submodule, implement `|expand` placeholder substitution, nested correlation, and percentile aggregation, add dashboard tiles for sigma_beam metrics, and clean up repo debris.

**Architecture:** sigma-beam-backend becomes a git submodule at `./sigma_beam`. Python features (expand, nested, percentile) are implemented directly in the submodule using TDD. Placeholders resolve at load time via explicit AST walking (not pySigma's ProcessingPipeline which is unreliable for this). Nested correlations run as a second stage inside `CorrelationFanout` — first-level alerts are converted to event dicts and fed through nested rules' transforms. Percentile is a Beaver extension (`type: percentile` with `beaver.percentile` annotations). Dashboard tiles use Pub/Sub topic-level metrics (no custom metric setup needed).

**Tech Stack:** Python 3.10+, pySigma ≥0.11, Apache Beam 2.55+, pytest, Hypothesis (fuzz), Rust (dashboard only).

---

## File Structure

### Beaver repo (Rust side)
| File | Responsibility |
|------|---------------|
| `sigma_beam/` (submodule) | Pin sigma-beam-backend at specific commit |
| `src/lib/dashboard.rs` (modify) | Add alerts/min + DLQ depth tiles |
| `.gitignore` (modify) | Add `output.log` |
| `output.log` (delete) | Remove accidentally committed debug log |

### sigma_beam package (Python — inside submodule)
| File | Responsibility |
|------|---------------|
| `src/sigma_beam/placeholders.py` (create) | Resolve `%name%` tokens in detection items using a user-supplied dict |
| `src/sigma_beam/loader.py` (modify) | Accept `placeholders` param, apply before compilation; two-pass correlation classification + cycle detection |
| `src/sigma_beam/conditions.py` (modify) | Safety-net for unresolved `SigmaQueryExpression` |
| `src/sigma_beam/ruleset.py` (modify) | Add `nested_correlation` list, `correlations_by_id()`, `percentile`/`percentile_field` fields, `is_nested` flag |
| `src/sigma_beam/correlation/fanout.py` (modify) | Register percentile; second-stage dispatch for nested correlations |
| `src/sigma_beam/correlation/nested.py` (create) | `alert_to_event()` + `AlertsToEvents` PTransform |
| `src/sigma_beam/correlation/percentile.py` (create) | `PercentileCorrelation` PTransform + `_PercentileCombineFn` |
| `tests/test_placeholders.py` (create) | Placeholder resolution unit + integration tests |
| `tests/test_modifier_matrix.py` (modify) | Convert xfail to passing test |
| `tests/test_ruleset_nested.py` (create) | Ruleset data model tests |
| `tests/test_loader_nested.py` (create) | Two-pass loader + cycle detection tests |
| `tests/test_correlation_nested.py` (create) | AlertToEvent + fanout wiring tests |
| `tests/test_percentile.py` (create) | CombineFn + integration tests |
| `tests/test_e2e_new_features.py` (create) | Combined e2e smoke on DirectRunner |
| `tests/fixtures/rules/nested_correlation.yml` (create) | Multi-rule fixture |
| `tests/fixtures/rules/percentile_latency.yml` (create) | Percentile fixture |
| `README.md` (modify) | Update feature table |

---

## Task 1: Pin sigma-beam-backend as Git Submodule

**Files:**
- Create: `sigma_beam/` (submodule)
- Modify: `.gitmodules`

- [ ] **Step 1: Add the submodule**

```bash
cd /Users/divkov/workplace/Beaver
git submodule add https://github.com/DIvkov575/sigma-beam-backend.git sigma_beam
```

- [ ] **Step 2: Verify sigma.rs path resolves**

```bash
ls sigma_beam/pyproject.toml
```
Expected: file exists. The existing code in `src/lib/sigma.rs` joins `CARGO_MANIFEST_DIR` with `"sigma_beam"` — this now resolves to the submodule directory.

- [ ] **Step 3: Verify Rust tests still pass**

```bash
cargo test 2>&1 | tail -5
```
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add .gitmodules sigma_beam
git commit -m "chore: pin sigma-beam-backend as git submodule"
```

---

## Task 2: Placeholder Resolution Module

**Files:**
- Create: `sigma_beam/src/sigma_beam/placeholders.py`
- Create: `sigma_beam/tests/test_placeholders.py`

- [ ] **Step 1: Write failing test**

```python
# sigma_beam/tests/test_placeholders.py
import textwrap
import pytest
from sigma.rule import SigmaRule
from sigma_beam.placeholders import resolve_placeholders
from sigma_beam.conditions import compile_rule


def _rule(body: str) -> SigmaRule:
    return SigmaRule.from_yaml(textwrap.dedent(body))


def test_expand_resolves_placeholder_table():
    r = _rule("""
        title: admin login
        logsource: {product: x}
        detection:
            sel: {User|expand: '%admin_users%'}
            condition: sel
    """)
    table = {"admin_users": ["alice", "bob", "root"]}
    resolved = resolve_placeholders(r, table)
    p = compile_rule(resolved)
    assert p({"User": "alice"})
    assert p({"User": "bob"})
    assert p({"User": "root"})
    assert not p({"User": "mallory"})


def test_expand_unknown_placeholder_raises():
    r = _rule("""
        title: t
        logsource: {product: x}
        detection:
            sel: {User|expand: '%unknown_group%'}
            condition: sel
    """)
    with pytest.raises(ValueError, match="unknown_group"):
        resolve_placeholders(r, {})
```

- [ ] **Step 2: Run to verify failure**

```bash
cd sigma_beam && pytest tests/test_placeholders.py -v
```
Expected: FAIL — `ModuleNotFoundError: No module named 'sigma_beam.placeholders'`

- [ ] **Step 3: Implement**

```python
# sigma_beam/src/sigma_beam/placeholders.py
"""Resolve |expand placeholders at load time using a user-supplied table.

pySigma's |expand modifier emits SigmaExpansion nodes when it can resolve
the placeholder, but leaves them as literal strings when it can't. We walk
the rule's detection items and replace %name% patterns with SigmaExpansion
nodes from the table BEFORE condition compilation.
"""

from __future__ import annotations

import re
from copy import deepcopy

from sigma.rule import SigmaRule
from sigma.types import SigmaExpansion, SigmaString

_PLACEHOLDER_RE = re.compile(r"^%([a-zA-Z0-9_]+)%$")


def resolve_placeholders(
    rule: SigmaRule, table: dict[str, list[str]]
) -> SigmaRule:
    """Return a copy of `rule` with all %placeholder% values expanded.

    Raises ValueError if a placeholder references a name not in `table`.
    """
    if not table:
        _check_no_placeholders(rule, table)
        return rule

    rule = deepcopy(rule)
    for detection_item in _iter_detection_items(rule):
        for i, val in enumerate(detection_item.value):
            if isinstance(val, SigmaString) and not val.contains_special():
                plain = val.to_plain()
                m = _PLACEHOLDER_RE.match(plain)
                if m:
                    name = m.group(1)
                    if name not in table:
                        raise ValueError(
                            f"placeholder '{name}' not found in table "
                            f"(available: {sorted(table.keys())})"
                        )
                    expanded = SigmaExpansion(
                        [SigmaString(v) for v in table[name]]
                    )
                    detection_item.value[i] = expanded
    return rule


def _check_no_placeholders(rule: SigmaRule, table: dict) -> None:
    for detection_item in _iter_detection_items(rule):
        for val in detection_item.value:
            if isinstance(val, SigmaString) and not val.contains_special():
                m = _PLACEHOLDER_RE.match(val.to_plain())
                if m:
                    raise ValueError(
                        f"placeholder '{m.group(1)}' not found in table "
                        f"(available: {sorted(table.keys())})"
                    )


def _iter_detection_items(rule: SigmaRule):
    """Yield all SigmaDetectionItem objects from a rule's detections."""
    from sigma.detection import SigmaDetection, SigmaDetectionItem
    for det in rule.detection.detections.values():
        if isinstance(det, SigmaDetectionItem):
            yield det
        elif isinstance(det, SigmaDetection):
            for item in det.detection_items:
                if isinstance(item, SigmaDetectionItem):
                    yield item
                elif hasattr(item, "detection_items"):
                    for sub in item.detection_items:
                        if isinstance(sub, SigmaDetectionItem):
                            yield sub
```

- [ ] **Step 4: Run tests**

```bash
cd sigma_beam && pytest tests/test_placeholders.py -v
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd sigma_beam && git add src/sigma_beam/placeholders.py tests/test_placeholders.py
git commit -m "feat: add |expand placeholder resolution module"
```

---

## Task 3: Wire Placeholders into Loader + Safety Net

**Files:**
- Modify: `sigma_beam/src/sigma_beam/loader.py`
- Modify: `sigma_beam/src/sigma_beam/conditions.py`
- Modify: `sigma_beam/tests/test_placeholders.py`
- Modify: `sigma_beam/tests/test_modifier_matrix.py`

- [ ] **Step 1: Write failing integration test**

Append to `sigma_beam/tests/test_placeholders.py`:
```python
from pathlib import Path
from sigma_beam.loader import load_from_dir


def test_loader_resolves_placeholders(tmp_path: Path):
    rule_file = tmp_path / "admin_rule.yml"
    rule_file.write_text("""
title: admin login
id: test-expand-001
logsource:
    product: app
    service: auth
detection:
    sel:
        User|expand: '%admin_users%'
    condition: sel
level: high
""")
    table = {"admin_users": ["alice", "bob"]}
    rs = load_from_dir(tmp_path, placeholders=table)
    assert len(rs.single_event) == 1
    p = rs.single_event[0].predicate
    assert p({"User": "alice"})
    assert p({"User": "bob"})
    assert not p({"User": "mallory"})
```

- [ ] **Step 2: Run to verify failure**

```bash
cd sigma_beam && pytest tests/test_placeholders.py::test_loader_resolves_placeholders -v
```
Expected: FAIL — `TypeError: load_from_dir() got an unexpected keyword argument 'placeholders'`

- [ ] **Step 3: Thread placeholders through loader.py**

In `sigma_beam/src/sigma_beam/loader.py`:

Add import at top:
```python
from .placeholders import resolve_placeholders
```

Modify `load_from_paths` signature to accept `placeholders: dict[str, list[str]] | None = None`.

After `apply_pipelines(rule, pipeline_selector)` loop, before the compilation loop, add:
```python
    if placeholders:
        for i, rule in enumerate(collection.rules):
            if isinstance(rule, SigmaRule):
                collection.rules[i] = resolve_placeholders(rule, placeholders)
```

Modify `load_from_dir` and `load_from_gcs` signatures to accept and pass through `placeholders`.

- [ ] **Step 4: Add safety-net in conditions.py**

In `sigma_beam/src/sigma_beam/conditions.py`, before the final `raise UnsupportedCondition` in `_match_value()`, add:
```python
    value_type = type(value).__name__
    if "Query" in value_type or "Placeholder" in value_type:
        raise UnsupportedCondition(
            f"unresolved placeholder/query expression ({value_type}); "
            "supply a placeholders table to load_from_dir/load_from_gcs"
        )
```

- [ ] **Step 5: Convert xfail in test_modifier_matrix.py**

Remove the `@pytest.mark.xfail` decorator from `test_expand_placeholder` and replace the test body:
```python
def test_expand_placeholder():
    """With a placeholder table supplied via the loader, |expand rules work."""
    from pathlib import Path
    from sigma_beam.loader import load_from_dir
    import tempfile

    rule_yaml = """\
title: admin detection
id: expand-test-001
logsource: {product: x}
detection:
    sel:
        User|expand: '%admin_users%'
    condition: sel
level: medium
"""
    with tempfile.TemporaryDirectory() as td:
        (Path(td) / "rule.yml").write_text(rule_yaml)
        rs = load_from_dir(td, placeholders={"admin_users": ["alice", "bob"]})
    p = rs.single_event[0].predicate
    assert p({"User": "alice"})
    assert p({"User": "bob"})
    assert not p({"User": "mallory"})
```

- [ ] **Step 6: Run full test suite**

```bash
cd sigma_beam && pytest -v 2>&1 | tail -20
```
Expected: all pass, no xfail for expand.

- [ ] **Step 7: Commit**

```bash
cd sigma_beam && git add -A
git commit -m "feat: wire placeholders into loader + safety net in conditions"
```

---

## Task 4: Ruleset Changes for Nested Correlation + Percentile

**Files:**
- Modify: `sigma_beam/src/sigma_beam/ruleset.py`
- Create: `sigma_beam/tests/test_ruleset_nested.py`

- [ ] **Step 1: Write failing test**

```python
# sigma_beam/tests/test_ruleset_nested.py
from sigma_beam.ruleset import CompiledCorrelation, CompiledRule, Ruleset


def test_ruleset_has_nested_correlation_list():
    rs = Ruleset()
    assert hasattr(rs, "nested_correlation")
    assert rs.nested_correlation == []


def test_correlations_by_id():
    c1 = CompiledCorrelation(
        id="corr-1", title="c1", severity="high", kind="event_count",
        referenced_rule_ids=("r1",), group_by=("user",),
        window_seconds=60, threshold=5,
    )
    rs = Ruleset(correlation=[c1])
    by_id = rs.correlations_by_id()
    assert by_id["corr-1"] is c1


def test_compiled_correlation_has_percentile_fields():
    c = CompiledCorrelation(
        id="c1", title="t", severity="high", kind="percentile",
        referenced_rule_ids=("r1",), group_by=("svc",),
        window_seconds=60, threshold=500,
        percentile=95.0, percentile_field="latency_ms",
    )
    assert c.percentile == 95.0
    assert c.percentile_field == "latency_ms"


def test_compiled_correlation_has_is_nested():
    c = CompiledCorrelation(
        id="c1", title="t", severity="high", kind="event_count",
        referenced_rule_ids=("corr-x",), group_by=("user",),
        window_seconds=60, threshold=3, is_nested=True,
    )
    assert c.is_nested is True
```

- [ ] **Step 2: Run to verify failure**

```bash
cd sigma_beam && pytest tests/test_ruleset_nested.py -v
```
Expected: FAIL — missing fields.

- [ ] **Step 3: Implement**

Replace `sigma_beam/src/sigma_beam/ruleset.py`:
```python
"""Compiled rule + correlation dataclasses + Ruleset container."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable

from .conditions import Predicate

CorrelationKind = str  # event_count, value_count, temporal, temporal_ordered, percentile


@dataclass(frozen=True)
class CompiledRule:
    id: str
    title: str
    severity: str
    predicate: Predicate
    source_path: str | None = None


@dataclass(frozen=True)
class CompiledCorrelation:
    id: str
    title: str
    severity: str
    kind: CorrelationKind
    referenced_rule_ids: tuple[str, ...]
    group_by: tuple[str, ...]
    window_seconds: int
    # event_count / value_count only:
    threshold: int | None = None
    threshold_op: str = "gte"   # gte | gt | lt | lte | eq
    # value_count only:
    value_field: str | None = None
    # percentile only:
    percentile: float | None = None
    percentile_field: str | None = None
    # temporal_ordered only (sequence of rule ids):
    ordered_sequence: tuple[str, ...] = ()
    # Nested correlation flag:
    is_nested: bool = False
    # Annotations:
    allowed_lateness_seconds: int = 300
    allow_high_cardinality: bool = False
    source_path: str | None = None


@dataclass
class Ruleset:
    single_event: list[CompiledRule] = field(default_factory=list)
    correlation: list[CompiledCorrelation] = field(default_factory=list)
    nested_correlation: list[CompiledCorrelation] = field(default_factory=list)

    def rules_by_id(self) -> dict[str, CompiledRule]:
        return {r.id: r for r in self.single_event}

    def correlations_by_id(self) -> dict[str, CompiledCorrelation]:
        return {c.id: c for c in self.correlation}
```

- [ ] **Step 4: Run tests**

```bash
cd sigma_beam && pytest tests/test_ruleset_nested.py -v
```
Expected: PASS.

- [ ] **Step 5: Run full suite for regressions**

```bash
cd sigma_beam && pytest -x -q
```
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
cd sigma_beam && git add src/sigma_beam/ruleset.py tests/test_ruleset_nested.py
git commit -m "feat(ruleset): add nested_correlation, correlations_by_id, percentile fields"
```

---

## Task 5: Two-Pass Loader with Cycle Detection

**Files:**
- Modify: `sigma_beam/src/sigma_beam/loader.py`
- Create: `sigma_beam/tests/test_loader_nested.py`
- Create: `sigma_beam/tests/fixtures/rules/nested_correlation.yml`

- [ ] **Step 1: Create test fixture**

Write `sigma_beam/tests/fixtures/rules/nested_correlation.yml`:
```yaml
---
title: failed login (base)
id: rule-failed-login
logsource:
    product: app
    service: auth
detection:
    sel:
        event: login
        status: failed
    condition: sel
level: low
---
title: brute force (first-level)
id: corr-brute-force
type: event_count
rules:
    - rule-failed-login
group-by:
    - src_ip
timespan: 5m
condition:
    gte: 10
level: high
---
title: distributed brute force (nested)
id: corr-distributed-brute
type: event_count
rules:
    - corr-brute-force
group-by:
    - target_user
timespan: 15m
condition:
    gte: 3
level: critical
```

- [ ] **Step 2: Write failing test**

```python
# sigma_beam/tests/test_loader_nested.py
from pathlib import Path
import pytest
from sigma_beam.loader import load_from_dir


def test_nested_correlation_from_fixture(tmp_path: Path):
    fixture = Path(__file__).parent / "fixtures" / "rules" / "nested_correlation.yml"
    (tmp_path / "rules.yml").write_text(fixture.read_text())
    rs = load_from_dir(tmp_path)
    assert len(rs.single_event) == 1
    assert len(rs.correlation) == 1
    assert len(rs.nested_correlation) == 1
    nested = rs.nested_correlation[0]
    assert nested.id == "corr-distributed-brute"
    assert nested.is_nested is True
    assert nested.referenced_rule_ids == ("corr-brute-force",)


def test_cycle_detection_raises(tmp_path: Path):
    (tmp_path / "cycle.yml").write_text("""
---
title: rule A
id: corr-a
type: event_count
rules: [corr-b]
group-by: [user]
timespan: 5m
condition: {gte: 5}
level: high
---
title: rule B
id: corr-b
type: event_count
rules: [corr-a]
group-by: [user]
timespan: 5m
condition: {gte: 5}
level: high
""")
    with pytest.raises(Exception, match="[Cc]ycle"):
        load_from_dir(tmp_path)
```

- [ ] **Step 3: Run to verify failure**

```bash
cd sigma_beam && pytest tests/test_loader_nested.py -v
```
Expected: FAIL — nested correlations placed in `rs.correlation`, not `rs.nested_correlation`.

- [ ] **Step 4: Implement two-pass compile + cycle detection**

In `sigma_beam/src/sigma_beam/loader.py`, add cycle detection helper at module level:
```python
def _detect_cycles(correlations: list[CompiledCorrelation]) -> None:
    """Raise RuleLoadError if the correlation reference graph has a cycle."""
    ids = {c.id for c in correlations}
    adj: dict[str, set[str]] = {c.id: set() for c in correlations}
    for c in correlations:
        for ref in c.referenced_rule_ids:
            if ref in ids:
                adj[c.id].add(ref)

    WHITE, GRAY, BLACK = 0, 1, 2
    color = {cid: WHITE for cid in ids}

    def dfs(node: str) -> None:
        color[node] = GRAY
        for nb in adj[node]:
            if color[nb] == GRAY:
                raise RuleLoadError(
                    f"cycle detected in correlation rules: {node} → {nb}"
                )
            if color[nb] == WHITE:
                dfs(nb)
        color[node] = BLACK

    for cid in ids:
        if color[cid] == WHITE:
            dfs(cid)
```

Replace the compilation loop at the end of `load_from_paths` with a two-pass approach:
```python
    # --- Two-pass compilation ---
    # Pass 1: compile all rules.
    all_correlations: list[CompiledCorrelation] = []
    for rule in collection.rules:
        src = sources[0] if len(sources) == 1 else None
        if isinstance(rule, SigmaCorrelationRule):
            c = _compile_correlation(rule, src)
            _lint_correlation(c)
            all_correlations.append(c)
        elif isinstance(rule, SigmaRule):
            rs.single_event.append(_compile_single(rule, src, logsource_filter))
        else:
            log.warning("skipping unknown rule type %s", type(rule).__name__)

    # Pass 2: separate first-level from nested, detect cycles.
    corr_ids = {c.id for c in all_correlations}
    _detect_cycles(all_correlations)

    import dataclasses
    for c in all_correlations:
        if any(r in corr_ids for r in c.referenced_rule_ids):
            rs.nested_correlation.append(dataclasses.replace(c, is_nested=True))
        else:
            rs.correlation.append(c)

    return rs
```

- [ ] **Step 5: Run tests**

```bash
cd sigma_beam && pytest tests/test_loader_nested.py -v
```
Expected: PASS.

- [ ] **Step 6: Run full suite**

```bash
cd sigma_beam && pytest -x -q
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
cd sigma_beam && git add -A
git commit -m "feat(loader): two-pass compilation with nested detection + cycle guard"
```

---

## Task 6: AlertToEvent Adapter + Nested Wiring in Fanout

**Files:**
- Create: `sigma_beam/src/sigma_beam/correlation/nested.py`
- Modify: `sigma_beam/src/sigma_beam/correlation/fanout.py`
- Create: `sigma_beam/tests/test_correlation_nested.py`

- [ ] **Step 1: Write failing test for adapter**

```python
# sigma_beam/tests/test_correlation_nested.py
import pytest
from sigma_beam.alerts import Alert
from sigma_beam.correlation.nested import alert_to_event


def test_alert_to_event_maps_fields():
    a = Alert(
        rule_id="corr-brute-force",
        rule_title="Brute Force",
        severity="high",
        fired_at="2026-01-01T00:00:00+00:00",
        window_start="2026-01-01T00:00:00+00:00",
        window_end="2026-01-01T00:05:00+00:00",
        correlation_key="10.0.0.1",
    )
    event = alert_to_event(a)
    assert event["rule_id"] == "corr-brute-force"
    assert event["severity"] == "high"
    assert event["timestamp"] == "2026-01-01T00:00:00+00:00"
    assert event["_sigma_beam_alert"] is True
```

- [ ] **Step 2: Run to verify failure**

```bash
cd sigma_beam && pytest tests/test_correlation_nested.py::test_alert_to_event_maps_fields -v
```
Expected: FAIL — no module.

- [ ] **Step 3: Implement the adapter**

Write `sigma_beam/src/sigma_beam/correlation/nested.py`:
```python
"""Convert Alert PCollection into event-shaped dicts for nested correlation."""

from __future__ import annotations

import apache_beam as beam

from ..alerts import Alert


def alert_to_event(alert: Alert) -> dict:
    """Map an Alert to a flat event dict suitable for correlation processing."""
    return {
        "rule_id": alert.rule_id,
        "rule_title": alert.rule_title,
        "severity": alert.severity,
        "timestamp": alert.fired_at,
        "window_start": alert.window_start,
        "window_end": alert.window_end,
        "correlation_key": alert.correlation_key,
        "_sigma_beam_alert": True,
    }


class AlertsToEvents(beam.PTransform):
    """PCollection[Alert] → PCollection[dict] for nested correlation input."""

    def expand(self, pcoll):
        return pcoll | "AlertToEvent" >> beam.Map(alert_to_event)
```

- [ ] **Step 4: Modify fanout.py for nested dispatch**

Replace `sigma_beam/src/sigma_beam/correlation/fanout.py` with:
```python
"""Dispatch each correlation rule in a Ruleset to the right per-kind transform
and flatten all resulting Alert PCollections into a single output.

Nested correlations (correlation-of-correlations) run as a second stage:
first-level alerts are converted to event dicts and fed through the nested
rules' transforms."""

from __future__ import annotations

import apache_beam as beam

from ..ruleset import CompiledCorrelation, CompiledRule, Ruleset
from .event_count import EventCountCorrelation
from .nested import AlertsToEvents
from .temporal import TemporalCorrelation
from .temporal_ordered import TemporalOrderedCorrelation
from .value_count import ValueCountCorrelation

_KIND_TO_TRANSFORM = {
    "event_count": EventCountCorrelation,
    "value_count": ValueCountCorrelation,
    "temporal": TemporalCorrelation,
    "temporal_ordered": TemporalOrderedCorrelation,
}


class UnknownCorrelationKind(Exception):
    pass


class CorrelationFanout(beam.PTransform):
    """PCollection[dict] → PCollection[Alert] across every correlation rule."""

    def __init__(self, ruleset: Ruleset) -> None:
        super().__init__()
        self._rs = ruleset

    def expand(self, pcoll):
        rs = self._rs
        by_id = rs.rules_by_id()

        branches = []

        # --- First-level correlations (reference single-event rules) ---
        for c in rs.correlation:
            cls = _KIND_TO_TRANSFORM.get(c.kind)
            if cls is None:
                raise UnknownCorrelationKind(c.kind)
            refs = [by_id[r] for r in c.referenced_rule_ids if r in by_id]
            if not refs:
                continue
            branches.append(
                pcoll | f"Correlation[{c.id}]" >> cls(c, refs)
            )

        if not branches and not rs.nested_correlation:
            return pcoll | "EmptyFanout" >> beam.Filter(lambda _: False) \
                         | "AsAlert" >> beam.Map(lambda _: None)

        if not branches:
            return pcoll | "EmptyFanout" >> beam.Filter(lambda _: False) \
                         | "AsAlert" >> beam.Map(lambda _: None)

        first_level_alerts = branches | "FlattenFirstLevel" >> beam.Flatten()

        # --- Nested correlations (reference other correlation rules) ---
        if rs.nested_correlation:
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

        return first_level_alerts


def _build_nested_refs(
    nc: CompiledCorrelation, corr_by_id: dict[str, CompiledCorrelation]
) -> list[CompiledRule]:
    """Build synthetic CompiledRule objects that match alert-events by rule_id."""
    refs = []
    for ref_id in nc.referenced_rule_ids:
        if ref_id not in corr_by_id:
            continue

        def _make_pred(rid: str):
            def pred(event: dict) -> bool:
                return event.get("rule_id") == rid
            return pred

        refs.append(CompiledRule(
            id=ref_id,
            title=f"(nested ref: {ref_id})",
            severity="internal",
            predicate=_make_pred(ref_id),
        ))
    return refs
```

- [ ] **Step 5: Write integration test for nested fanout**

Append to `sigma_beam/tests/test_correlation_nested.py`:
```python
import apache_beam as beam
from apache_beam.testing.test_pipeline import TestPipeline

from sigma_beam.correlation.fanout import CorrelationFanout
from sigma_beam.ruleset import CompiledCorrelation, CompiledRule, Ruleset


def _always_true(e: dict) -> bool:
    return e.get("event") == "login" and e.get("status") == "failed"


def test_nested_fanout_does_not_crash():
    """Nested pipeline doesn't crash — structural smoke test."""
    base_rule = CompiledRule(
        id="rule-failed-login", title="failed login",
        severity="low", predicate=_always_true,
    )
    child_corr = CompiledCorrelation(
        id="corr-brute-force", title="brute force", severity="high",
        kind="event_count", referenced_rule_ids=("rule-failed-login",),
        group_by=("src_ip",), window_seconds=300, threshold=2,
    )
    nested_corr = CompiledCorrelation(
        id="corr-distributed", title="distributed brute", severity="critical",
        kind="event_count", referenced_rule_ids=("corr-brute-force",),
        group_by=("correlation_key",), window_seconds=900, threshold=2,
        is_nested=True,
    )
    rs = Ruleset(
        single_event=[base_rule],
        correlation=[child_corr],
        nested_correlation=[nested_corr],
    )

    events = [
        {"event": "login", "status": "failed", "src_ip": f"10.0.0.{i}",
         "timestamp": 1000 + j}
        for i in range(3) for j in range(5)
    ]

    with TestPipeline() as p:
        pcoll = p | beam.Create(events)
        _ = pcoll | CorrelationFanout(rs)
    # No crash = success for structural test
```

- [ ] **Step 6: Run tests**

```bash
cd sigma_beam && pytest tests/test_correlation_nested.py -v
```
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
cd sigma_beam && git add -A
git commit -m "feat(correlation): nested fanout with AlertToEvent adapter"
```

---

## Task 7: Percentile Correlation PTransform

**Files:**
- Create: `sigma_beam/src/sigma_beam/correlation/percentile.py`
- Create: `sigma_beam/tests/test_percentile.py`
- Create: `sigma_beam/tests/fixtures/rules/percentile_latency.yml`

- [ ] **Step 1: Write failing test**

```python
# sigma_beam/tests/test_percentile.py
import pytest
import apache_beam as beam
from apache_beam.testing.test_pipeline import TestPipeline
from apache_beam.testing.util import assert_that, is_not_empty, equal_to

from sigma_beam.correlation.percentile import PercentileCorrelation, _PercentileCombineFn
from sigma_beam.ruleset import CompiledCorrelation, CompiledRule


def _always_true(e: dict) -> bool:
    return True


def test_percentile_combine_fn():
    fn = _PercentileCombineFn(percentile=95.0)
    acc = fn.create_accumulator()
    for v in range(100):
        acc = fn.add_input(acc, float(v))
    result = fn.extract_output(acc)
    assert 93.0 <= result <= 96.0


def test_percentile_fires_when_exceeded():
    base = CompiledRule(id="r1", title="r", severity="low", predicate=_always_true)
    corr = CompiledCorrelation(
        id="corr-p95", title="high latency p95", severity="high",
        kind="percentile", referenced_rule_ids=("r1",),
        group_by=("service",), window_seconds=60,
        threshold=500, threshold_op="gte",
        percentile=95.0, percentile_field="latency_ms",
    )
    events = [
        {"service": "api", "latency_ms": 100, "timestamp": 1000 + i}
        for i in range(95)
    ] + [
        {"service": "api", "latency_ms": 1000, "timestamp": 1000 + i}
        for i in range(95, 100)
    ]

    with TestPipeline() as p:
        pcoll = p | beam.Create(events)
        alerts = pcoll | PercentileCorrelation(corr, [base])
        assert_that(alerts, is_not_empty())


def test_percentile_does_not_fire_when_below():
    base = CompiledRule(id="r1", title="r", severity="low", predicate=_always_true)
    corr = CompiledCorrelation(
        id="corr-p95", title="low latency", severity="high",
        kind="percentile", referenced_rule_ids=("r1",),
        group_by=("service",), window_seconds=60,
        threshold=500, threshold_op="gte",
        percentile=95.0, percentile_field="latency_ms",
    )
    events = [
        {"service": "api", "latency_ms": 100, "timestamp": 1000 + i}
        for i in range(100)
    ]

    with TestPipeline() as p:
        pcoll = p | beam.Create(events)
        alerts = pcoll | PercentileCorrelation(corr, [base])
        assert_that(alerts, equal_to([]))
```

- [ ] **Step 2: Run to verify failure**

```bash
cd sigma_beam && pytest tests/test_percentile.py -v
```
Expected: FAIL — no module `sigma_beam.correlation.percentile`

- [ ] **Step 3: Implement**

Write `sigma_beam/src/sigma_beam/correlation/percentile.py`:
```python
"""percentile correlation: fire when the Nth percentile of a field's values
exceeds a threshold within a window, grouped by `group_by`.

Uses a sorted-list accumulator capped at MAX_SAMPLES per key per window.
"""

from __future__ import annotations

import math

import apache_beam as beam
from apache_beam.transforms.window import FixedWindows
from apache_beam.transforms.trigger import (
    AccumulationMode, AfterWatermark, Repeatedly,
)

from ..alerts import Alert
from ..field_access import MISSING, get_field
from ..ruleset import CompiledCorrelation, CompiledRule
from ._common import (
    attach_event_time, cmp_threshold, make_group_key,
    passes_any_referenced_rule,
)

MAX_SAMPLES = 50_000


class _PercentileCombineFn(beam.CombineFn):
    def __init__(self, percentile: float) -> None:
        self._percentile = percentile

    def create_accumulator(self):
        return []

    def add_input(self, acc: list, val: float):
        if len(acc) < MAX_SAMPLES:
            acc.append(val)
        return acc

    def merge_accumulators(self, accs):
        out = []
        for a in accs:
            out.extend(a)
            if len(out) >= MAX_SAMPLES:
                return out[:MAX_SAMPLES]
        return out

    def extract_output(self, acc: list) -> float:
        if not acc:
            return 0.0
        acc.sort()
        k = (self._percentile / 100.0) * (len(acc) - 1)
        f = math.floor(k)
        c = math.ceil(k)
        if f == c:
            return acc[int(k)]
        return acc[f] * (c - k) + acc[c] * (k - f)


class _ToWindowedAlert(beam.DoFn):
    def __init__(self, c: CompiledCorrelation) -> None:
        self._c = c

    def process(self, element, window=beam.DoFn.WindowParam):
        key, pval = element
        c = self._c
        if not cmp_threshold(pval, c.threshold_op, c.threshold or 0):
            return
        yield Alert(
            rule_id=c.id,
            rule_title=c.title,
            severity=c.severity,
            window_start=window.start.to_utc_datetime().isoformat(),
            window_end=window.end.to_utc_datetime().isoformat(),
            correlation_key=key,
        )


class PercentileCorrelation(beam.PTransform):
    def __init__(self, c: CompiledCorrelation, refs: list[CompiledRule]) -> None:
        super().__init__()
        if not c.percentile_field:
            raise ValueError(f"percentile rule {c.id!r} missing percentile_field")
        if c.percentile is None:
            raise ValueError(f"percentile rule {c.id!r} missing percentile value")
        self._c = c
        self._refs = refs

    def expand(self, pcoll):
        c = self._c
        refs = self._refs
        pf = c.percentile_field

        def extract_value(e: dict) -> float:
            return float(get_field(e, pf))

        return (
            pcoll
            | "Filter" >> beam.Filter(lambda e: passes_any_referenced_rule(e, refs))
            | "DropMissing" >> beam.Filter(lambda e: get_field(e, pf) is not MISSING)
            | "AttachTs" >> beam.Map(attach_event_time)
            | "Key" >> beam.Map(lambda e: (make_group_key(e, c.group_by), extract_value(e)))
            | "Window" >> beam.WindowInto(
                FixedWindows(c.window_seconds),
                allowed_lateness=c.allowed_lateness_seconds,
                trigger=Repeatedly(AfterWatermark()),
                accumulation_mode=AccumulationMode.DISCARDING,
            )
            | "Percentile" >> beam.CombinePerKey(_PercentileCombineFn(c.percentile))
            | "Threshold" >> beam.ParDo(_ToWindowedAlert(c))
        )
```

- [ ] **Step 4: Run tests**

```bash
cd sigma_beam && pytest tests/test_percentile.py -v
```
Expected: PASS.

- [ ] **Step 5: Create fixture rule**

Write `sigma_beam/tests/fixtures/rules/percentile_latency.yml`:
```yaml
---
title: any request
id: rule-any-request
logsource:
    product: app
    service: http
detection:
    sel:
        event: request
    condition: sel
level: informational
---
title: P95 latency spike
id: corr-p95-latency
type: event_count
rules:
    - rule-any-request
group-by:
    - service
timespan: 5m
condition:
    gte: 500
beaver:
    percentile: 95.0
    percentile_field: latency_ms
level: high
```

- [ ] **Step 6: Commit**

```bash
cd sigma_beam && git add -A
git commit -m "feat: percentile correlation PTransform"
```

---

## Task 8: Register Percentile in Fanout + Loader

**Files:**
- Modify: `sigma_beam/src/sigma_beam/correlation/fanout.py`
- Modify: `sigma_beam/src/sigma_beam/loader.py`
- Modify: `sigma_beam/tests/test_percentile.py`

- [ ] **Step 1: Write failing loader test**

Append to `sigma_beam/tests/test_percentile.py`:
```python
from pathlib import Path
from sigma_beam.loader import load_from_dir


def test_loader_parses_percentile_rule(tmp_path: Path):
    (tmp_path / "rules.yml").write_text("""
---
title: any req
id: rule-req
logsource: {product: app, service: http}
detection:
    sel: {event: request}
    condition: sel
level: low
---
title: p95 spike
id: corr-p95
type: event_count
rules: [rule-req]
group-by: [service]
timespan: 5m
condition:
    gte: 500
beaver:
    percentile: 95.0
    percentile_field: latency_ms
level: high
""")
    rs = load_from_dir(tmp_path)
    assert len(rs.correlation) == 1
    c = rs.correlation[0]
    assert c.kind == "percentile"
    assert c.percentile == 95.0
    assert c.percentile_field == "latency_ms"
    assert c.threshold == 500
```

- [ ] **Step 2: Run to verify failure**

```bash
cd sigma_beam && pytest tests/test_percentile.py::test_loader_parses_percentile_rule -v
```
Expected: FAIL — loader doesn't extract percentile fields from annotations.

- [ ] **Step 3: Update loader's _compile_correlation**

In `sigma_beam/src/sigma_beam/loader.py`, modify `_compile_correlation` to extract percentile:
```python
    # Percentile fields from beaver annotations
    percentile = _annotation(rule, "percentile", None)
    percentile_field = _annotation(rule, "percentile_field", None)
    if percentile is not None:
        percentile = float(percentile)
        kind = "percentile"  # Override kind for beaver-extension rules
```

And pass `percentile=percentile, percentile_field=percentile_field` to the `CompiledCorrelation` constructor.

- [ ] **Step 4: Register percentile in fanout**

In `sigma_beam/src/sigma_beam/correlation/fanout.py`, add:
```python
from .percentile import PercentileCorrelation

_KIND_TO_TRANSFORM = {
    "event_count": EventCountCorrelation,
    "value_count": ValueCountCorrelation,
    "temporal": TemporalCorrelation,
    "temporal_ordered": TemporalOrderedCorrelation,
    "percentile": PercentileCorrelation,
}
```

- [ ] **Step 5: Run all percentile tests**

```bash
cd sigma_beam && pytest tests/test_percentile.py -v
```
Expected: PASS.

- [ ] **Step 6: Run full suite**

```bash
cd sigma_beam && pytest -x -q
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
cd sigma_beam && git add -A
git commit -m "feat: wire percentile into loader + fanout dispatch"
```

---

## Task 9: End-to-End Smoke Test

**Files:**
- Create: `sigma_beam/tests/test_e2e_new_features.py`

- [ ] **Step 1: Write combined e2e test**

```python
# sigma_beam/tests/test_e2e_new_features.py
"""End-to-end tests for expand, nested correlation, and percentile on DirectRunner."""

from pathlib import Path
import tempfile

import apache_beam as beam
from apache_beam.testing.test_pipeline import TestPipeline
from apache_beam.testing.util import assert_that, is_not_empty

from sigma_beam.correlation.fanout import CorrelationFanout
from sigma_beam.loader import load_from_dir
from sigma_beam.single_event import SingleEventDetect, MAIN


def test_e2e_expand_placeholder():
    with tempfile.TemporaryDirectory() as td:
        (Path(td) / "rule.yml").write_text("""
title: admin action
id: rule-admin
logsource: {product: app}
detection:
    sel:
        User|expand: '%admins%'
        action: delete
    condition: sel
level: high
""")
        rs = load_from_dir(td, placeholders={"admins": ["root", "admin"]})

    events = [
        {"User": "root", "action": "delete"},
        {"User": "nobody", "action": "delete"},
        {"User": "admin", "action": "read"},
    ]
    with TestPipeline() as p:
        pcoll = p | beam.Create(events)
        results = pcoll | SingleEventDetect(rs.single_event)
        assert_that(results[MAIN], is_not_empty())


def test_e2e_percentile_correlation():
    with tempfile.TemporaryDirectory() as td:
        (Path(td) / "rules.yml").write_text("""
---
title: any req
id: r1
logsource: {product: app}
detection:
    sel: {type: request}
    condition: sel
level: low
---
title: p99 latency
id: corr-p99
type: event_count
rules: [r1]
group-by: [svc]
timespan: 1m
condition:
    gte: 1000
beaver:
    percentile: 99.0
    percentile_field: ms
level: critical
""")
        rs = load_from_dir(td)

    events = [
        {"type": "request", "svc": "api", "ms": 100, "timestamp": 1000 + i}
        for i in range(99)
    ] + [{"type": "request", "svc": "api", "ms": 5000, "timestamp": 1099}]

    with TestPipeline() as p:
        pcoll = p | beam.Create(events)
        alerts = pcoll | CorrelationFanout(rs)
        assert_that(alerts, is_not_empty())
```

- [ ] **Step 2: Run**

```bash
cd sigma_beam && pytest tests/test_e2e_new_features.py -v
```
Expected: PASS.

- [ ] **Step 3: Run full suite as final validation**

```bash
cd sigma_beam && pytest -v 2>&1 | tail -20
```
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
cd sigma_beam && git add tests/test_e2e_new_features.py
git commit -m "test: e2e smoke tests for expand, nested, and percentile"
```

---

## Task 10: Update README

**Files:**
- Modify: `sigma_beam/README.md`

- [ ] **Step 1: Update the feature table**

Change:
```markdown
| `|expand` placeholder substitution | ❌ (documented gap) |
| Correlation aliases / percentile / nested correlation | ❌ |
```

To:
```markdown
| `|expand` placeholder substitution | ✅ (requires placeholder table) |
| Correlation: nested (correlation-of-correlations) | ✅ (one level) |
| Correlation: percentile aggregation | ✅ (beaver extension) |
| Correlation aliases | ❌ |
```

- [ ] **Step 2: Add placeholder usage example**

After "Quick start" section:
```markdown
## Placeholder tables

Rules using `|expand` require a placeholder table at load time:

```python
from sigma_beam.loader import load_from_dir

rs = load_from_dir("rules/", placeholders={
    "admin_users": ["alice", "bob", "root"],
    "critical_hosts": ["dc01", "dc02", "ca01"],
})
```
```

- [ ] **Step 3: Commit**

```bash
cd sigma_beam && git add README.md
git commit -m "docs: update feature table for expand, nested, percentile"
```

---

## Task 11: Dashboard Tiles for sigma_beam Metrics (Rust)

**Files:**
- Modify: `src/lib/dashboard.rs`

- [ ] **Step 1: Add sigma_beam tiles**

In `src/lib/dashboard.rs`, locate the `render_dashboard` function's tile construction section. After the existing tiles, add:

```rust
    // --- sigma_beam metric tiles ---
    // Alerts per minute (Pub/Sub publish rate on beaver-alerts topic)
    tiles.push(format!(r#"{{
      "title": "Alerts / min (sigma_beam)",
      "xyChart": {{
        "dataSets": [{{
          "timeSeriesQuery": {{
            "timeSeriesFilter": {{
              "filter": "resource.type=\"pubsub_topic\" AND resource.labels.topic_id=\"beaver-alerts\" AND metric.type=\"pubsub.googleapis.com/topic/send_message_operation_count\"",
              "aggregation": {{ "alignmentPeriod": "60s", "perSeriesAligner": "ALIGN_RATE" }}
            }}
          }}
        }}]
      }}
    }}"#));

    // DLQ depth (undelivered messages on beaver-dlq)
    tiles.push(format!(r#"{{
      "title": "DLQ Depth",
      "xyChart": {{
        "dataSets": [{{
          "timeSeriesQuery": {{
            "timeSeriesFilter": {{
              "filter": "resource.type=\"pubsub_topic\" AND resource.labels.topic_id=\"beaver-dlq\" AND metric.type=\"pubsub.googleapis.com/topic/num_undelivered_messages\"",
              "aggregation": {{ "alignmentPeriod": "60s", "perSeriesAligner": "ALIGN_MEAN" }}
            }}
          }}
        }}]
      }}
    }}"#));
```

Note: The exact insertion depends on the tile array variable name. Find the `Vec<String>` used for tile widgets and append.

- [ ] **Step 2: Build**

```bash
cargo build 2>&1 | tail -5
```
Expected: success.

- [ ] **Step 3: Run tests**

```bash
cargo test 2>&1 | tail -10
```
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src/lib/dashboard.rs
git commit -m "feat(dashboard): add sigma_beam alerts + DLQ tiles"
```

---

## Task 12: Cleanup & Final Verification

**Files:**
- Delete: `output.log`
- Modify: `.gitignore`
- Update: `sigma_beam/` (submodule pointer)

- [ ] **Step 1: Remove output.log**

```bash
git rm output.log
```

- [ ] **Step 2: Add to .gitignore**

Append `output.log` to `.gitignore`.

- [ ] **Step 3: Push sigma_beam commits and update submodule pointer**

```bash
cd sigma_beam && git push origin main
cd /Users/divkov/workplace/Beaver
git add sigma_beam
```

- [ ] **Step 4: Run full Rust test suite**

```bash
cargo test 2>&1 | tail -10
```
Expected: all pass.

- [ ] **Step 5: Run full Python test suite**

```bash
cd sigma_beam && pytest -v 2>&1 | tail -20
```
Expected: all pass, 0 xfail for expand.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: remove output.log, update sigma_beam submodule pointer"
```

---

## Out of Scope

- **`--placeholders_json` CLI flag** — trivial to add to `correlation_pipeline.py` later.
- **Correlation aliases** (Sigma's `aliases:` field) — separate feature.
- **T-digest for percentile** — sorted-list is fine for ≤50k samples/key/window.
- **Nested-of-nested (>2 levels)** — DAG validation permits it but fanout does one nesting level only.
- **In-flight rule reload** — Rules baked at job launch; edits require `beaver deploy`.

## Risks

- **pySigma's `SigmaDetectionItem` internal API** for placeholder resolution may shift between 0.11 minor releases. Pin `~=0.11` and test.
- **Nested correlation event-time** — second stage sees alerts with `fired_at` timestamps, not original event times. Window alignment may need tuning.
- **Percentile accuracy** degrades when MAX_SAMPLES is hit; late arrivals are dropped silently. Consider a Beam counter for dropped samples.
- **State cost** — Nested correlation doubles stateful transforms. Acceptable given cardinality linting.

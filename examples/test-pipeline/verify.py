"""
Replay every payload under `payloads/` through the compiled `detections()`
function and confirm the firing rules match `payloads/expected.yaml`.

Usage:
    /tmp/beaver-test/detections/venv/bin/python examples/test-pipeline/verify.py

Loads the `detections_gen.py` that `cargo run -- deploy` produces. Run a
deploy (or `./scripts/e2e_test_pipeline.sh`) once to populate it.
"""
from __future__ import annotations

import ast
import json
import logging
import sys
from pathlib import Path

import yaml


HERE = Path(__file__).parent
CONFIG = HERE / "beaver_config"
PAYLOADS = HERE / "payloads"
GENERATED = CONFIG / "artifacts" / "detections_gen.py"
EXPECTED = PAYLOADS / "expected.yaml"


def _build_detections_namespace() -> dict:
    """Pull the inner FunctionDefs out of run() so detections() is callable
    without launching Beam."""
    src = GENERATED.read_text()
    tree = ast.parse(src)
    run_fn = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run")
    body = [n for n in run_fn.body if isinstance(n, ast.FunctionDef)]
    mod = ast.Module(body=body, type_ignores=[])
    ast.fix_missing_locations(mod)
    ns = {"__name__": "__verify__", "json": json, "logging": logging}
    exec(compile(mod, str(GENERATED), "exec"), ns)
    return ns


class CapturingHandler(logging.Handler):
    def __init__(self):
        super().__init__()
        self.records: list[dict] = []
        self.drop_events: list[dict] = []

    def emit(self, record: logging.LogRecord) -> None:
        msg = record.getMessage()
        if not msg.startswith("{"):
            return
        try:
            payload = json.loads(msg)
        except json.JSONDecodeError:
            return
        if payload.get("event") == "BEAVER_SIEM_MATCH":
            self.records.append(payload)
        elif payload.get("event") == "BEAVER_SIEM_DROP":
            self.drop_events.append(payload)


def replay_payload(ns: dict, raw: bytes) -> tuple[set[str], bool]:
    """Returns (rule_names_that_fired, was_dropped)."""
    handler = CapturingHandler()
    root = logging.getLogger()
    prev_level = root.level
    root.addHandler(handler)
    root.setLevel(logging.WARNING)
    try:
        # Mirror what process() does in the generated module.
        try:
            record = json.loads(raw.decode("utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError):
            return set(), True
        ns["detections"](record)
        return {r["rule_name"] for r in handler.records}, False
    finally:
        root.removeHandler(handler)
        root.setLevel(prev_level)


def main() -> int:
    if not GENERATED.exists():
        sys.exit(f"missing {GENERATED}; run `cargo run -- deploy --path {CONFIG}` (or ./scripts/e2e_test_pipeline.sh) once")

    expected = yaml.safe_load(EXPECTED.read_text())
    ns = _build_detections_namespace()

    failures: list[str] = []
    total = 0
    for rel, manifest in expected.items():
        path = PAYLOADS / rel
        if not path.exists():
            failures.append(f"{rel}: payload missing on disk")
            continue
        total += 1

        expected_matches = set(manifest.get("matches", []))
        expected_drop = bool(manifest.get("drop", False))

        actual_matches, actual_drop = replay_payload(ns, path.read_bytes())

        ok = actual_matches == expected_matches and actual_drop == expected_drop
        status = "PASS" if ok else "FAIL"
        print(f"  [{status}] {rel}")
        if not ok:
            failures.append(
                f"{rel}: expected matches={sorted(expected_matches)} "
                f"drop={expected_drop}, got matches={sorted(actual_matches)} "
                f"drop={actual_drop}"
            )

    print()
    print(f"{total - len(failures)}/{total} payloads behaved as expected")
    if failures:
        print("FAILURES:")
        for f in failures:
            print(f"  - {f}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

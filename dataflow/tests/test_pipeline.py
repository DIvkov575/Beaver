"""
Tests that the generated detections_gen.py operates as a real Apache Beam
pipeline. Two layers:

  * Logic layer: extract the inner detection functions from `run()` and
    invoke them directly on dicts. Fast; no Beam runtime needed.

  * Pipeline layer: feed synthetic PubsubMessages through Beam's DirectRunner
    using TestPipeline, asserting the harness logs the correct rule names.
    This proves the PTransform graph composes and `process()` parses + dispatches.
"""
import ast
import logging

import apache_beam as beam
import pytest
from apache_beam.io.gcp.pubsub import PubsubMessage
from apache_beam.options.pipeline_options import PipelineOptions


def _extract_inner_runtime(module):
    """Pulls the inner FunctionDefs out of run() into an executable namespace."""
    tree = ast.parse(module._source_text)
    run_fn = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run")
    body = [n for n in run_fn.body if isinstance(n, ast.FunctionDef)]
    mod = ast.Module(body=body, type_ignores=[])
    ast.fix_missing_locations(mod)
    ns = {"__name__": "__synthetic__", "json": __import__("json"), "logging": logging}
    exec(compile(mod, module._source_path, "exec"), ns)
    return ns


# ---- logic layer ----------------------------------------------------------


def test_detection_dispatcher_calls_each_rule(compiled_detections):
    ns = _extract_inner_runtime(compiled_detections)
    assert callable(ns.get("detections")), "compiled module must define detections()"
    # Inspect the dispatcher's body to confirm it calls every rule.
    src = compiled_detections._source_text
    tree = ast.parse(src)
    run_fn = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run")
    rule_fns = [n.name for n in run_fn.body
                if isinstance(n, ast.FunctionDef) and n.name not in ("harness", "process", "detections")]
    dispatcher = next(n for n in run_fn.body
                      if isinstance(n, ast.FunctionDef) and n.name == "detections")
    called = [n.value.func.id for n in dispatcher.body if isinstance(n, ast.Expr)]
    assert sorted(called) == sorted(rule_fns), (
        f"dispatcher must call every rule. rules={rule_fns}, called={called}"
    )


def test_matching_record_emits_warning(compiled_detections, caplog):
    import json as _json
    ns = _extract_inner_runtime(compiled_detections)
    record = {"event": "login", "status": "failed", "user": "alice@neon-circle.io"}
    with caplog.at_level(logging.WARNING):
        ns["detections"](record)
    matches = [_json.loads(r.message) for r in caplog.records if r.message.startswith("{")]
    rule_names = [m.get("rule_name") for m in matches]
    assert "failed_login" in rule_names, f"expected failed_login match in {rule_names}"


def test_filtered_record_does_not_emit_warning(compiled_detections, caplog):
    """external_iam_grant has `selection AND NOT internal_user` — internal
    grantees should be filtered out and not emit a warning."""
    import json as _json
    ns = _extract_inner_runtime(compiled_detections)
    record = {"event": "iam_change", "action": "grant",
              "role": "roles/viewer", "granted_to": "new@neon-circle.io"}
    with caplog.at_level(logging.WARNING):
        ns["detections"](record)
    matches = [_json.loads(r.message) for r in caplog.records if r.message.startswith("{")]
    triggered = [m for m in matches if m.get("rule_name") == "external_iam_grant"]
    assert not triggered, f"internal grant should not match external_iam_grant: {triggered}"


def test_non_matching_record_silent(compiled_detections, caplog):
    ns = _extract_inner_runtime(compiled_detections)
    record = {"event": "logout", "user": "alice"}
    with caplog.at_level(logging.WARNING):
        ns["detections"](record)
    assert not caplog.records, f"unexpected warnings: {[r.message for r in caplog.records]}"


# ---- pipeline layer (Beam DirectRunner) -----------------------------------


def _build_process_only(compiled_module):
    """Build a process() function backed by the same dispatcher, suitable for
    feeding into a Beam pipeline. We can't call run() directly (it constructs
    a real Pub/Sub source), so we re-create the closure pieces here."""
    ns = _extract_inner_runtime(compiled_module)
    return ns["process"]


def _direct_pipeline():
    """Plain Pipeline + DirectRunner with explicit options so we don't read
    sys.argv (which would trigger DetectionsOptions' required --subscription)."""
    opts = PipelineOptions(["--runner=DirectRunner", "--subscription=test-sub"])
    return beam.Pipeline(options=opts)


def test_beam_directrunner_processes_each_message(compiled_detections, caplog):
    """Real Beam DirectRunner pipeline: 3 messages, exactly one should match.
    Uses the test-pipeline rule set's `failed_login`."""
    import json as _json
    process = _build_process_only(compiled_detections)
    messages = [
        PubsubMessage(data=b'{"event":"login","status":"failed","user":"alice@neon-circle.io"}', attributes={}),
        PubsubMessage(data=b'{"event":"login","status":"success","user":"alice@neon-circle.io"}', attributes={}),
        PubsubMessage(data=b'{"event":"data_access","operation":"read","user":"bob@neon-circle.io"}', attributes={}),
    ]
    with caplog.at_level(logging.WARNING):
        with _direct_pipeline() as p:
            (p
             | "Inject" >> beam.Create(messages)
             | "RunDetections" >> beam.Map(process))

    payloads = [_json.loads(r.message) for r in caplog.records if r.message.startswith("{")]
    matches = [p for p in payloads if p.get("event") == "BEAVER_SIEM_MATCH"]
    assert len(matches) == 1, f"expected 1 match, got {matches}"
    assert matches[0]["rule_name"] == "failed_login"


def test_beam_directrunner_skips_non_json(compiled_detections, caplog):
    import json as _json
    process = _build_process_only(compiled_detections)
    messages = [PubsubMessage(data=b"not json at all", attributes={})]
    with caplog.at_level(logging.WARNING):
        with _direct_pipeline() as p:
            p | beam.Create(messages) | beam.Map(process)
    payloads = [_json.loads(r.message) for r in caplog.records if r.message.startswith("{")]
    drops = [p for p in payloads if p.get("event") == "BEAVER_SIEM_DROP"]
    assert len(drops) == 1, f"expected 1 drop event, got {drops}"

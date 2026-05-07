"""
Pytest fixture that compiles a fresh `detections_gen.py` from this repo's
sigma rules + detections template via `cargo run -- compile`, then loads the
result so each test can exercise it.

Reuses an existing sigma venv (which has pysigma + apache-beam) instead of
creating a new one, so the fixture stays fast. The venv path is read from
`BEAVER_TEST_VENV`, defaulting to `/tmp/beaver-sample/detections/venv`.
"""
import importlib.util
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_VENV = "/tmp/beaver-sample/detections/venv"


@pytest.fixture(scope="session")
def compiled_detections(tmp_path_factory):
    """Compiles a sample config dir and returns the loaded detections_gen module."""
    venv = Path(os.environ.get("BEAVER_TEST_VENV", DEFAULT_VENV))
    if not venv.exists():
        pytest.skip(f"sigma venv missing at {venv}; set BEAVER_TEST_VENV")

    work = tmp_path_factory.mktemp("beaver_compile")
    detections = work / "detections"
    detections.mkdir()
    (detections / "input").mkdir()
    (detections / "output").mkdir()
    (work / "artifacts").mkdir()

    # Copy assets that init/embed normally provides.
    shutil.copy(REPO_ROOT / "src" / "beaver_config" / "detections" / "sigma_generate.py",
                detections / "sigma_generate.py")
    shutil.copy(REPO_ROOT / "dataflow" / "detections_template.py",
                detections / "detections_template.py")
    # Symlink the existing venv so we don't pip-install from scratch.
    os.symlink(venv, detections / "venv")

    # Two distinct rules so the dispatcher has multiple entries.
    shutil.copy(REPO_ROOT / "src" / "beaver_config" / "detections" / "input" / "se.yml",
                detections / "input" / "gcp_policy.yml")
    (detections / "input" / "login.yml").write_text(
        "title: Suspicious Login\n"
        "id: 7c5e1a92-9c1b-4d4d-a311-d9b2e5e84a02\n"
        "status: experimental\n"
        "logsource: {product: gcp, service: auth}\n"
        "detection:\n"
        "  selection: {event: login, severity: high}\n"
        "  filter: {user|startswith: 'service-'}\n"
        "  condition: selection and not filter\n"
        "level: high\n"
    )

    subprocess.run(
        ["cargo", "run", "--quiet", "--", "compile", "--path", str(work)],
        cwd=REPO_ROOT, check=True, capture_output=True,
    )

    gen = work / "artifacts" / "detections_gen.py"
    spec = importlib.util.spec_from_file_location("detections_gen", gen)
    module = importlib.util.module_from_spec(spec)
    sys.argv = ["detections_gen"]  # so PipelineOptions doesn't see pytest's argv
    spec.loader.exec_module(module)
    module._source_text = gen.read_text()
    module._source_path = str(gen)
    return module

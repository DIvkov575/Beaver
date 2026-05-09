"""
Loads `examples/test-pipeline/beaver_config/artifacts/detections_gen.py` —
the same file `cargo run -- deploy` produces — for the dataflow tests to
exercise. If the file is missing, the tests skip with a hint.
"""
import importlib.util
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
GENERATED = REPO_ROOT / "examples" / "test-pipeline" / "beaver_config" / "artifacts" / "detections_gen.py"


@pytest.fixture(scope="session")
def compiled_detections():
    if not GENERATED.exists():
        pytest.skip(
            f"{GENERATED} missing; run `cargo run -- deploy --path "
            f"examples/test-pipeline/beaver_config` (or ./scripts/e2e_test_pipeline.sh) once."
        )

    spec = importlib.util.spec_from_file_location("detections_gen", GENERATED)
    module = importlib.util.module_from_spec(spec)
    sys.argv = ["detections_gen"]  # so PipelineOptions doesn't see pytest's argv
    spec.loader.exec_module(module)
    module._source_text = GENERATED.read_text()
    module._source_path = str(GENERATED)
    return module

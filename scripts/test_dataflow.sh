#!/usr/bin/env bash
# Runs the Apache Beam dataflow tests in dataflow/tests/.
# Reuses an existing sigma venv (apache-beam + pysigma) instead of building one.
# Override the venv location via BEAVER_TEST_VENV.
set -eu
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/.." && pwd)

VENV=${BEAVER_TEST_VENV:-/tmp/beaver-test/detections/venv}
if [ ! -d "$VENV" ]; then
  echo "sigma venv missing at $VENV" >&2
  echo "  set BEAVER_TEST_VENV or run a beaver init/deploy first to create one" >&2
  exit 1
fi

# Ensure pytest is installed in the venv.
"$VENV/bin/python" -c "import pytest" 2>/dev/null || \
  "$VENV/bin/pip" install -q pytest

cd "$ROOT"
BEAVER_TEST_VENV="$VENV" "$VENV/bin/python" -m pytest dataflow/tests/ "$@"

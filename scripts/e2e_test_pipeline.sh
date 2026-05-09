#!/usr/bin/env bash
# Full end-to-end test of Beaver against the examples/test-pipeline fixture:
#   1. Provision input pubsub topic + sub
#   2. cargo run -- deploy   (creates BQ, pubsub output, GCS, image, CRS, dataflow)
#   3. Wait for Dataflow workers to start
#   4. Publish every payload from examples/test-pipeline/payloads/
#   5. Wait for end-to-end propagation (Vector + Dataflow)
#   6. Verify: BQ row count >= valid-payload count
#              Dataflow detection events match expected.yaml per-rule counts
#   7. cargo run -- destroy + teardown input
#
# Trap-based cleanup ensures GCP resources don't leak even if assertions fail.
# Override defaults via env vars (BEAVER_TEST_PROJECT, BEAVER_DATAFLOW_ZONE, etc).
set -uo pipefail

PROJECT=${BEAVER_TEST_PROJECT:-neon-circle-400322}
REGION=${BEAVER_TEST_REGION:-us-east1}
ZONE=${BEAVER_DATAFLOW_ZONE:-us-east1-d}
INPUT_TOPIC=${BEAVER_E2E_INPUT_TOPIC:-beaver-test-pipeline-input}
INPUT_SUB=${BEAVER_E2E_INPUT_SUB:-beaver-test-pipeline-input-sub}
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/.." && pwd)
CONFIG_PATH=${BEAVER_E2E_CONFIG:-examples/test-pipeline/beaver_config}
PAYLOADS=${BEAVER_E2E_PAYLOADS:-examples/test-pipeline/payloads}
EXPECTED="$PAYLOADS/expected.yaml"
WORKER_WAIT=${BEAVER_E2E_WORKER_WAIT:-180}
PROCESS_WAIT=${BEAVER_E2E_PROCESS_WAIT:-180}
PYTHON=${BEAVER_E2E_PYTHON:-/tmp/beaver-test/detections/venv/bin/python}

cd "$ROOT"

DEPLOYED=false
INPUT_PROVISIONED=false
declare -i FAILED=0

log()  { printf '\n=== %s ===\n' "$*"; }
pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*" >&2; FAILED+=1; }

cleanup() {
    log "cleanup"
    if [ "$DEPLOYED" = true ]; then
        cargo run --quiet -- destroy --path "$CONFIG_PATH" 2>&1 | tail -3 || true
    fi
    if [ "$INPUT_PROVISIONED" = true ]; then
        gcloud pubsub subscriptions delete "$INPUT_SUB" --project="$PROJECT" --quiet 2>/dev/null || true
        gcloud pubsub topics delete "$INPUT_TOPIC" --project="$PROJECT" --quiet 2>/dev/null || true
    fi
}
trap cleanup EXIT

require() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "missing $1 on PATH" >&2
        exit 2
    }
}
require gcloud
require bq
require cargo
require "$PYTHON"

# Sanity: the configured input subscription must match what beaver_config expects.
expected_sub=$(grep -E '^[[:space:]]+subscription:' "$CONFIG_PATH/beaver_config.yaml" \
    | head -1 | sed -E 's/.*subscription:[[:space:]]*//; s/"//g; s/[[:space:]]*$//')
if [ "$expected_sub" != "$INPUT_SUB" ]; then
    echo "config expects subscription=$expected_sub but INPUT_SUB=$INPUT_SUB" >&2
    echo "set BEAVER_E2E_INPUT_SUB=$expected_sub or update the fixture" >&2
    exit 2
fi

log "provision input topic + sub"
gcloud pubsub topics create "$INPUT_TOPIC" --project="$PROJECT"
gcloud pubsub subscriptions create "$INPUT_SUB" --topic="$INPUT_TOPIC" --project="$PROJECT"
INPUT_PROVISIONED=true

log "deploy beaver (BEAVER_DATAFLOW_ZONE=$ZONE)"
BEAVER_DATAFLOW_ZONE="$ZONE" cargo run --quiet -- deploy --path "$CONFIG_PATH"
DEPLOYED=true

PIPELINE_NAME=$(grep '^dataflow_pipeline_name:' "$CONFIG_PATH/artifacts/resources.yaml" \
    | awk '{print $2}')
DATASET=$(grep '^[[:space:]]*dataset_id:' "$CONFIG_PATH/artifacts/resources.yaml" \
    | head -1 | awk '{print $2}')
echo "  pipeline=$PIPELINE_NAME dataset=$DATASET"

if [ -z "$PIPELINE_NAME" ]; then
    fail "deploy did not record dataflow_pipeline_name; check output.log"
    exit 1
fi

JOB_ID=$(gcloud dataflow jobs list --region="$REGION" --project="$PROJECT" \
    --filter="name=$PIPELINE_NAME" --format="value(JOB_ID)" | head -1)
echo "  dataflow JOB_ID=$JOB_ID"

log "wait ${WORKER_WAIT}s for Dataflow workers"
sleep "$WORKER_WAIT"

state=$(gcloud dataflow jobs describe "$JOB_ID" --region="$REGION" --project="$PROJECT" \
    --format="value(currentState)")
echo "  dataflow state: $state"
if [ "$state" != "JOB_STATE_RUNNING" ]; then
    fail "Dataflow job not running (state=$state); aborting verification"
    exit 1
fi

log "publish payloads"
declare -i PUBLISHED=0
declare -i VALID_JSON=0
for f in "$PAYLOADS"/{benign,single_match,multi_match,edge}/*; do
    rel=${f#"$PAYLOADS/"}
    body=$(cat "$f")
    gcloud pubsub topics publish "$INPUT_TOPIC" --project="$PROJECT" \
        --message="$body" >/dev/null
    ((PUBLISHED++))
    if "$PYTHON" -c "import json,sys; json.loads(sys.stdin.read())" >/dev/null 2>&1 <<<"$body"; then
        ((VALID_JSON++))
    fi
    echo "  published: $rel"
done
echo "  total published: $PUBLISHED ($VALID_JSON valid JSON)"

log "wait ${PROCESS_WAIT}s for vector→bq + dataflow processing"
sleep "$PROCESS_WAIT"

log "verify BQ"
BQ_COUNT=$(bq query --nouse_legacy_sql --project_id="$PROJECT" --format=csv --quiet \
    "SELECT COUNT(*) FROM \`${PROJECT}.${DATASET}.table1\`" | tail -1)
echo "  bq rows: $BQ_COUNT"
if [ "$BQ_COUNT" -ge "$VALID_JSON" ]; then
    pass "BQ has $BQ_COUNT rows (expected >= $VALID_JSON)"
else
    fail "BQ has only $BQ_COUNT rows (expected >= $VALID_JSON)"
fi

log "verify Dataflow detections"
# Dataflow wraps Python logger output: jsonPayload.message is the literal
# string we logged (a JSON object). Filter via substring, then parse out
# rule_name client-side.
MESSAGES=$(gcloud logging read \
    "resource.type=dataflow_step AND resource.labels.job_id=$JOB_ID AND jsonPayload.message:\"BEAVER_SIEM_MATCH\"" \
    --project="$PROJECT" --limit=200 --format='value(jsonPayload.message)')
RULES=$(echo "$MESSAGES" | "$PYTHON" -c '
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        d = json.loads(line)
    except json.JSONDecodeError:
        continue
    name = d.get("rule_name")
    if name:
        print(name)
')
echo "$RULES" | "$PYTHON" "$HERE/e2e_aggregate_verify.py" "$EXPECTED"
agg_rc=$?
if [ $agg_rc -eq 0 ]; then
    pass "Dataflow detections match expected.yaml"
else
    fail "Dataflow detection counts mismatch"
fi

log "summary"
if [ $FAILED -eq 0 ]; then
    echo "ALL CHECKS PASSED"
    exit 0
else
    echo "$FAILED FAILURE(S)"
    exit 1
fi

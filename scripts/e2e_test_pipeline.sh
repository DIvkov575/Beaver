#!/usr/bin/env bash
# End-to-end test that mirrors a user using Beaver:
#   1. Provision the input topic the user will publish to.
#   2. cargo run -- deploy
#   3. Publish every test payload to the input topic.
#   4. Wait for Vector + Dataflow to process.
#   5. Read Dataflow detection events from Cloud Logging, compare to manifest.
#   6. cargo run -- destroy + remove the input topic.
#
# Cleanup runs on any exit (success, failure, ctrl-c) via `trap`.
set -euo pipefail

PROJECT=${BEAVER_TEST_PROJECT:-neon-circle-400322}
REGION=${BEAVER_TEST_REGION:-us-east1}
CONFIG=examples/test-pipeline/beaver_config
PAYLOADS=examples/test-pipeline/payloads
TOPIC=beaver-test-pipeline-input
SUB=beaver-test-pipeline-input-sub
PYTHON=${BEAVER_E2E_PYTHON:-/tmp/beaver-test/detections/venv/bin/python}

cleanup() {
    cargo run --quiet -- destroy --path "$CONFIG" 2>&1 | tail -1 || true
    gcloud pubsub subscriptions delete "$SUB" --project="$PROJECT" --quiet 2>/dev/null || true
    gcloud pubsub topics delete    "$TOPIC" --project="$PROJECT" --quiet 2>/dev/null || true
}
trap cleanup EXIT

# 1. Provision input.
gcloud pubsub topics create "$TOPIC" --project="$PROJECT"
gcloud pubsub subscriptions create "$SUB" --topic="$TOPIC" --project="$PROJECT"

# 2. Deploy.
BEAVER_DATAFLOW_ZONE=us-east1-d cargo run -- deploy --path "$CONFIG"

# 3. Publish every payload.
for f in "$PAYLOADS"/{benign,single_match,multi_match,edge}/*; do
    gcloud pubsub topics publish "$TOPIC" --project="$PROJECT" \
        --message="$(cat "$f")" >/dev/null
    echo "  published: ${f#$PAYLOADS/}"
done

# 4. Wait for Vector + Dataflow.
echo "waiting 240s for pipeline to drain..."
sleep 240

# 5. Read detection events from Cloud Logging, parse rule names, verify.
JOB=$(grep '^dataflow_pipeline_name:' "$CONFIG/artifacts/resources.yaml" | awk '{print $2}')
JOB_ID=$(gcloud dataflow jobs list --region="$REGION" --project="$PROJECT" \
    --filter="name=$JOB" --format='value(JOB_ID)' | head -1)

gcloud logging read \
    "resource.type=dataflow_step AND resource.labels.job_id=$JOB_ID AND jsonPayload.message:\"BEAVER_SIEM_MATCH\"" \
    --project="$PROJECT" --limit=200 --format='value(jsonPayload.message)' \
| "$PYTHON" -c '
import json, sys
for line in sys.stdin:
    if line.strip():
        try: print(json.loads(line)["rule_name"])
        except Exception: pass
' \
| "$PYTHON" scripts/e2e_aggregate_verify.py "$PAYLOADS/expected.yaml"

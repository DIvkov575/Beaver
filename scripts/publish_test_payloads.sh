#!/usr/bin/env bash
# Publishes every payload under examples/test-pipeline/payloads/ to the input
# topic. Run after the input topic exists (scripts/input_topic.sh) and Beaver
# has been deployed (cargo run -- deploy --path examples/test-pipeline/beaver_config).
set -euo pipefail

PROJECT=${BEAVER_TEST_PROJECT:-neon-circle-400322}
TOPIC=${BEAVER_INPUT_TOPIC:-beaver-test-pipeline-input}
PAYLOADS=${BEAVER_PAYLOADS:-examples/test-pipeline/payloads}

for f in "$PAYLOADS"/{benign,single_match,multi_match,edge}/*; do
    gcloud pubsub topics publish "$TOPIC" --project="$PROJECT" \
        --message="$(cat "$f")" >/dev/null
    echo "  published: ${f#"$PAYLOADS/"}"
done

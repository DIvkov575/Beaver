#!/usr/bin/env bash
# Publishes every payload under ../payloads/ to the input pubsub topic.
# Run after input_topic.sh and after `cargo run -- deploy --path ../beaver_config`.
set -euo pipefail
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
PAYLOADS="$HERE/../payloads"

PROJECT=${BEAVER_TEST_PROJECT:-neon-circle-400322}
TOPIC=${BEAVER_INPUT_TOPIC:-beaver-test-pipeline-input}

for f in "$PAYLOADS"/{benign,single_match,multi_match,edge}/*; do
    gcloud pubsub topics publish "$TOPIC" --project="$PROJECT" \
        --message="$(cat "$f")" >/dev/null
    echo "  published: ${f#"$PAYLOADS/"}"
done

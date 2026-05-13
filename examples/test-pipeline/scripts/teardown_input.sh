#!/usr/bin/env bash
set -u
PROJECT=${BEAVER_TEST_PROJECT:-neon-circle-400322}
TOPIC=${BEAVER_INPUT_TOPIC:-beaver-test-pipeline-input}
SUB=${BEAVER_INPUT_SUB:-beaver-test-pipeline-input-sub}

gcloud pubsub subscriptions delete "$SUB" --project="$PROJECT" --quiet 2>&1 | tail -1 || true
gcloud pubsub topics delete "$TOPIC" --project="$PROJECT" --quiet 2>&1 | tail -1 || true

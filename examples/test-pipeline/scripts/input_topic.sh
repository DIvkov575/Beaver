#!/usr/bin/env bash
set -eu
PROJECT=${BEAVER_TEST_PROJECT:-neon-circle-400322}
TOPIC=${BEAVER_INPUT_TOPIC:-beaver-test-pipeline-input}
SUB=${BEAVER_INPUT_SUB:-beaver-test-pipeline-input-sub}

if ! gcloud pubsub topics describe "$TOPIC" --project="$PROJECT" >/dev/null 2>&1; then
  gcloud pubsub topics create "$TOPIC" --project="$PROJECT"
else
  echo "input topic $TOPIC already exists"
fi

if ! gcloud pubsub subscriptions describe "$SUB" --project="$PROJECT" >/dev/null 2>&1; then
  gcloud pubsub subscriptions create "$SUB" --topic="$TOPIC" --project="$PROJECT"
else
  echo "input subscription $SUB already exists"
fi

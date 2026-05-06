#!/usr/bin/env bash
set -eu
PROJECT=${BEAVER_TEST_PROJECT:-neon-circle-400322}
TOPIC=${BEAVER_INPUT_TOPIC:-beaver-smoke-input}
N=${N:-5}

for i in $(seq 1 "$N"); do
  ts=$(date -u +%FT%TZ)
  payload="{\"event\":\"login\",\"user\":\"user_${i}\",\"ts\":\"${ts}\"}"
  gcloud pubsub topics publish "$TOPIC" --project="$PROJECT" --message="$payload" >/dev/null
  echo "published #$i: $payload"
done

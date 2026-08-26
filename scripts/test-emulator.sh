#!/usr/bin/env bash
set -euo pipefail

project_id="${FIREBASE_PROJECT_ID:-demo-rtdb-typed}"

if [[ "$project_id" != demo-* ]]; then
  echo "Refusing emulator tests for non-demo project: $project_id" >&2
  exit 2
fi

if ! command -v docker >/dev/null; then
  echo "docker is required for the port safety check" >&2
  exit 2
fi

echo "Docker port bindings currently in use:"
docker ps --format '  {{.Names}} -> {{.Ports}}'

if docker ps --format '{{.Ports}}' | grep -Eq '(^|[^0-9])(9000|4000)->'; then
  echo "Refusing to start Firebase: Docker already publishes port 9000 or 4000" >&2
  exit 2
fi

for port in 9000 4000; do
  if (command -v ss >/dev/null && ss -ltn | awk '{print $4}' | grep -Eq ":${port}$") || \
     (command -v lsof >/dev/null && lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1); then
    echo "Refusing to start Firebase: host port $port is already listening" >&2
    exit 2
  fi
done

if ! command -v npx >/dev/null; then
  echo "npx is required; install Node.js first" >&2
  exit 2
fi

echo "Starting Firebase Realtime Database emulator for $project_id on 127.0.0.1:9000"
npx --yes firebase-tools emulators:exec \
  --only database \
  --project "$project_id" \
  "cargo test --test emulator -- --ignored --skip emulator_stress_high_profile_manual"

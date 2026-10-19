#!/usr/bin/env bash
set -euo pipefail
script_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
acceptance_root=${ACCEPTANCE_ROOT:-/tmp/media88-native-acceptance-data}
project=${ACCEPTANCE_COMPOSE_PROJECT:-media88-native-acceptance}
mkdir -p "$acceptance_root/qdrant" "$acceptance_root/snapshots"
if [[ -e "$acceptance_root/stop" ]]; then
  printf 'Choose fresh ACCEPTANCE_ROOT: stop sentinel already exists.\n' >&2
  exit 1
fi
trap 'docker compose -p "$project" -f "$script_root/compose.yml" down' EXIT INT TERM
docker compose -p "$project" -f "$script_root/compose.yml" up -d
while [[ ! -f "$acceptance_root/stop" ]]; do sleep 1; done

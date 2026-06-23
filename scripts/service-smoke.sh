#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/service-smoke.sh [--disposable]

Runs a Docker Compose service-mode smoke check for api, web, and qdrant.

By default this uses the normal persisted Compose data directories and stops
only services started by the check. Pass --disposable to use temporary app and
Qdrant data directories that are removed after the check.
USAGE
}

disposable=0
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --disposable)
      disposable=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 64
      ;;
  esac
done

smoke_tmp=""
cleanup_tmp() {
  local status=$?
  trap - EXIT INT TERM

  if [[ -n "$smoke_tmp" ]]; then
    rm -rf "$smoke_tmp"
  fi

  exit "$status"
}

if [[ "$disposable" -eq 1 ]]; then
  running_services="$(docker compose ps --services --filter status=running api web qdrant 2>/dev/null || true)"
  if [[ -n "$running_services" ]]; then
    printf 'Refusing --disposable while Compose services are already running:\n%s\n' "$running_services" >&2
    printf 'Stop them first with `bun run service:down`, or run the smoke check without --disposable to reuse the active data directories.\n' >&2
    exit 69
  fi

  smoke_tmp="$(mktemp -d)"
  export APP_DATA_DIR="$smoke_tmp/app"
  export QDRANT_DATA_DIR="$smoke_tmp/qdrant"
  export QDRANT_SNAPSHOTS_DIR="$smoke_tmp/qdrant-snapshots"
  trap cleanup_tmp EXIT INT TERM
fi

web_port="${WEB_PORT:-5173}"
api_port="${API_PORT:-8000}"
web_url="http://127.0.0.1:${web_port}"
proxied_health_url="${web_url}/api/health"
direct_health_url="http://127.0.0.1:${api_port}/api/health"

wait_for_http() {
  local label="$1"
  local url="$2"
  local deadline=$((SECONDS + 90))

  printf 'Waiting for %s at %s\n' "$label" "$url" >&2
  until curl -fsS "$url" >/dev/null; do
    if ((SECONDS >= deadline)); then
      printf 'Timed out waiting for %s at %s\n' "$label" "$url" >&2
      docker compose ps api web qdrant >&2 || true
      return 1
    fi
    sleep 2
  done
}

run_smoke_checks() {
  wait_for_http "web UI" "$web_url"
  wait_for_http "proxied backend health" "$proxied_health_url"
  wait_for_http "direct backend health" "$direct_health_url"
  printf 'Service smoke check passed: %s, %s, %s\n' "$web_url" "$proxied_health_url" "$direct_health_url"
}

docker compose config >/dev/null

export COMPOSE_SERVICES="${COMPOSE_SERVICES:-api web qdrant}"
export SERVICE_SMOKE_WEB_URL="$web_url"
export SERVICE_SMOKE_PROXIED_HEALTH_URL="$proxied_health_url"
export SERVICE_SMOKE_DIRECT_HEALTH_URL="$direct_health_url"
bash scripts/run-with-compose-services.sh bash -c "$(declare -f wait_for_http run_smoke_checks); web_url=\"\$SERVICE_SMOKE_WEB_URL\"; proxied_health_url=\"\$SERVICE_SMOKE_PROXIED_HEALTH_URL\"; direct_health_url=\"\$SERVICE_SMOKE_DIRECT_HEALTH_URL\"; run_smoke_checks"

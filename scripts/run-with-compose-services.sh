#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

started_services=()
wrapper_tmp="$(mktemp -d)"
export TMPDIR="$wrapper_tmp"

running_services() {
  docker compose ps --services --filter status=running 2>/dev/null || true
}

wait_for_qdrant() {
  local url="${QDRANT_URL:-http://127.0.0.1:6333}"
  local ready_url="${url%/}/readyz"
  local deadline=$((SECONDS + 30))

  printf 'Waiting for Qdrant at %s\n' "$ready_url" >&2
  until curl -fsS "$ready_url" >/dev/null 2>&1; do
    if ((SECONDS >= deadline)); then
      printf 'Timed out waiting for Qdrant at %s\n' "$ready_url" >&2
      docker compose ps >&2 || true
      return 1
    fi
    sleep 1
  done
}

wait_for_started_services() {
  for service in "$@"; do
    case "$service" in
      qdrant)
        wait_for_qdrant
        ;;
    esac
  done
}

cleanup() {
  local status=$?
  trap - EXIT INT TERM

  if [[ -x ./scripts/ci/bootstrap-e2e-db.sh ]]; then
    ./scripts/ci/bootstrap-e2e-db.sh --teardown || true
  elif [[ "${#started_services[@]}" -gt 0 ]]; then
    docker compose stop "${started_services[@]}" >/dev/null || true
    docker compose rm -f "${started_services[@]}" >/dev/null || true
  fi

  rm -rf "$wrapper_tmp"
  exit "$status"
}

if [[ "$#" -eq 0 ]]; then
  echo "usage: $0 <command> [args...]" >&2
  exit 64
fi

trap cleanup EXIT INT TERM

if [[ -x ./scripts/ci/bootstrap-e2e-db.sh ]]; then
  ./scripts/ci/bootstrap-e2e-db.sh
else
  mapfile -t before < <(running_services)

  if [[ -n "${COMPOSE_SERVICES:-}" ]]; then
    # shellcheck disable=SC2086
    docker compose up -d $COMPOSE_SERVICES
    # shellcheck disable=SC2086
    wait_for_started_services $COMPOSE_SERVICES
  else
    docker compose up -d
    wait_for_started_services qdrant
  fi

  mapfile -t after < <(running_services)
  for service in "${after[@]}"; do
    already_running=0
    for existing in "${before[@]}"; do
      if [[ "$service" == "$existing" ]]; then
        already_running=1
        break
      fi
    done
    if [[ "$already_running" -eq 0 ]]; then
      started_services+=("$service")
    fi
  done
fi

"$@"

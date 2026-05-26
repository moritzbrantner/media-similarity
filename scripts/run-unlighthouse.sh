#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

port="${UNLIGHTHOUSE_PORT:-4179}"
site="${UNLIGHTHOUSE_SITE:-http://127.0.0.1:${port}}"
server_pid=""
server_log=""

cleanup() {
  local status=$?
  trap - EXIT INT TERM

  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi

  if [[ -n "$server_log" ]]; then
    rm -f "$server_log"
  fi

  exit "$status"
}

trap cleanup EXIT INT TERM

if [[ ! -f frontend/dist/index.html ]]; then
  echo "frontend/dist is missing; run \`bun run build\` before \`bun run test:perf\`." >&2
  exit 1
fi

if [[ -z "${UNLIGHTHOUSE_SITE:-}" ]]; then
  server_log="$(mktemp)"
  bun scripts/serve-unlighthouse-preview.mjs "$port" >"$server_log" 2>&1 &
  server_pid=$!

  ready=0
  for _ in {1..60}; do
    if curl -fsS "$site" >/dev/null 2>&1; then
      ready=1
      break
    fi

    if ! kill -0 "$server_pid" 2>/dev/null; then
      cat "$server_log" >&2
      exit 1
    fi

    sleep 0.5
  done

  if [[ "$ready" -ne 1 ]]; then
    cat "$server_log" >&2
    echo "Timed out waiting for frontend preview at ${site}" >&2
    exit 1
  fi
fi

bunx unlighthouse-ci --config-file unlighthouse.config.ts --site "$site"

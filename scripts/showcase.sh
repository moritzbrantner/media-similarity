#!/usr/bin/env bash
# One-command showcase: `bun run showcase` (this script without arguments).
#
# Checks prerequisites, prepares a separate showcase data directory, starts Qdrant via
# the detected Compose command, downloads the sample corpus when missing, starts the
# backend and the Vite UI with heavy model roles disabled, indexes the sample corpus,
# verifies a sample query and keeps running until interrupted.
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

# shellcheck source=scripts/lib/compose.sh
source "$project_root/scripts/lib/compose.sh"

resolve_path() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$project_root" "$1" ;;
  esac
}

sample_root="$(resolve_path "${SHOWCASE_SAMPLE_DIR:-sample-images/showcase}")"
sample_source_dir="$sample_root/sources"
sample_query_dir="$sample_root/queries"
api_url="${SHOWCASE_API_URL:-http://127.0.0.1:8000}"
api_url="${api_url%/}"
wait_seconds="${SHOWCASE_WAIT_SECONDS:-180}"
index_timeout_seconds="${SHOWCASE_INDEX_TIMEOUT_SECONDS:-1800}"
ui_port=5173
ui_url="http://127.0.0.1:${ui_port}"
qdrant_url="${QDRANT_URL:-http://127.0.0.1:6333}"

usage() {
  cat <<'USAGE'
Usage: bash scripts/showcase.sh [--check-prereqs|--seed-only|--check|--dev]

Without arguments (`bun run showcase`): check prerequisites, start Qdrant, the
backend and the Vite UI with heavy model roles disabled, download and index the
sample corpus, verify a sample query, then keep running until Ctrl-C.

Options:
  --check-prereqs  Only check the required tools (bun, cargo, ffmpeg, ffprobe and a
                   compose command) and the optional PDF tools. Builds nothing.
  --seed-only      Index the sample corpus through a running backend and verify a
                   sample query. Uses SHOWCASE_API_URL (default
                   http://127.0.0.1:8000), SHOWCASE_SAMPLE_DIR (default
                   sample-images/showcase) and SHOWCASE_WAIT_SECONDS.
  --check          Check prerequisites and the sample-corpus manifest, then print
                   sample-corpus .env values.
  --dev            Run --check, then start the regular development stack (bun dev).
  -h, --help       Show this help.
USAGE
}

# --- prerequisites -------------------------------------------------------------------

hint_for() {
  case "$1" in
    bun) printf 'install Bun from https://bun.sh (curl -fsSL https://bun.sh/install | bash)' ;;
    cargo) printf 'install the Rust toolchain with rustup from https://rustup.rs (curl --proto =https -sSf https://sh.rustup.rs | sh)' ;;
    ffmpeg) printf 'install FFmpeg, e.g. sudo apt install ffmpeg or brew install ffmpeg (https://ffmpeg.org/download.html)' ;;
    ffprobe) printf 'ffprobe ships with FFmpeg; install it, e.g. sudo apt install ffmpeg or brew install ffmpeg (https://ffmpeg.org/download.html)' ;;
    compose) printf 'install Docker with the Compose plugin (https://docs.docker.com/compose/install/) or Podman with podman-compose (https://podman.io/docs/installation)' ;;
  esac
}

report_missing() {
  printf 'missing: %s\nhint: %s\n' "$1" "$(hint_for "${2:-$1}")" >&2
}

check_prereqs() {
  local failed=0 tool

  for tool in bun cargo ffmpeg ffprobe; do
    if command -v "$tool" >/dev/null 2>&1; then
      printf 'ok: %s\n' "$tool"
    else
      report_missing "$tool"
      failed=1
    fi
  done

  if detect_compose_command; then
    printf 'ok: compose (%s)\n' "${COMPOSE_CMD[*]}"
  else
    report_missing "compose (docker compose, podman compose or podman-compose)" compose
    failed=1
  fi

  for tool in pdfinfo pdftoppm pdftotext; do
    if command -v "$tool" >/dev/null 2>&1; then
      printf 'ok: %s\n' "$tool"
    else
      printf 'warn: %s is not on PATH; PDF indexing and search stay unavailable (install poppler-utils)\n' "$tool" >&2
    fi
  done

  if [[ "$failed" -ne 0 ]]; then
    printf 'Install the missing tools above, then run bun run showcase again.\n' >&2
  fi
  return "$failed"
}

# --- --check / --dev (sample-corpus .env values) --------------------------------------

print_env_values() {
  cat <<ENV

Sample corpus .env values:
HOST_PICTURES_DIR=$sample_source_dir
HOST_VIDEO_DIR=$sample_source_dir
HOST_AUDIO_DIR=$sample_source_dir

Query files:
$sample_query_dir
ENV
}

check_showcase() {
  local failed=0

  check_prereqs || failed=1
  if [[ "$failed" -eq 0 ]]; then
    cargo run --manifest-path backend/Cargo.toml --bin sample_corpus -- check
  fi

  if [[ -d "$sample_source_dir" && -d "$sample_query_dir" ]]; then
    printf 'ok: sample showcase files are present\n'
  else
    printf 'warn: sample showcase files are not downloaded yet; run bun run showcase:download\n' >&2
  fi

  print_env_values

  return "$failed"
}

# --- seeding through the HTTP API ----------------------------------------------------

json_eval() {
  JSON_EXPR="$1" bun --eval '
const fs = require("fs");
const input = fs.readFileSync(0, "utf8");
const data = input.trim() ? JSON.parse(input) : null;
const result = Function("data", "return (" + process.env.JSON_EXPR + ");")(data);
if (result !== undefined && result !== null) console.log(String(result));
'
}

# PID of a locally started backend; the health wait gives up early when it exits.
backend_pid=""

wait_for_backend() {
  local deadline=$((SECONDS + wait_seconds))
  printf 'Waiting for the backend at %s\n' "$api_url" >&2
  until curl -fsS --max-time 2 "$api_url/api/health" >/dev/null 2>&1; do
    if [[ -n "$backend_pid" ]] && ! kill -0 "$backend_pid" 2>/dev/null; then
      printf 'error: the backend process exited before %s became reachable\n' "$api_url" >&2
      return 1
    fi
    if ((SECONDS >= deadline)); then
      printf 'error: backend at %s is not reachable after %ss (SHOWCASE_WAIT_SECONDS)\n' "$api_url" "$wait_seconds" >&2
      return 1
    fi
    sleep 1
  done
}

active_index_job_id() {
  curl -fsS --max-time 10 "$api_url/api/jobs" | json_eval '
(data ?? [])
  .filter((job) => String(job.spec?.kind ?? "").startsWith("index") && !["Succeeded", "Failed", "Cancelled"].includes(job.status))
  .map((job) => job.spec.id)[0]'
}

start_index_job() {
  local body_file="$1" code
  code="$(curl -sS --max-time 30 -o "$body_file" -w '%{http_code}' -X POST "$api_url/api/jobs/index")" || {
    printf 'error: could not start indexing at %s/api/jobs/index\n' "$api_url" >&2
    return 1
  }
  case "$code" in
    2??)
      json_eval 'data.spec.id' <"$body_file"
      ;;
    409)
      printf 'An indexing job is already running; waiting for it instead\n' >&2
      active_index_job_id
      ;;
    *)
      printf 'error: POST %s/api/jobs/index returned HTTP %s:\n' "$api_url" "$code" >&2
      cat "$body_file" >&2
      printf '\n' >&2
      return 1
      ;;
  esac
}

wait_for_index_job() {
  local job_id="$1" deadline=$((SECONDS + index_timeout_seconds)) body status
  printf 'Indexing the sample corpus (job %s)\n' "$job_id" >&2
  while true; do
    body="$(curl -fsS --max-time 10 "$api_url/api/jobs/$job_id")" || {
      printf 'error: could not read job %s from %s\n' "$job_id" "$api_url" >&2
      return 1
    }
    status="$(printf '%s' "$body" | json_eval 'data.status')"
    case "$status" in
      Succeeded) return 0 ;;
      Failed)
        # A job that indexed media but failed on some files (for example a PDF without
        # the optional Poppler tools) still leaves a searchable corpus; the sample query
        # below decides whether the showcase works.
        local indexed
        indexed="$(printf '%s' "$body" | json_eval 'Number(data.metadata?.indexed ?? 0) + Number(data.metadata?.already_indexed ?? 0)')"
        if [[ "$indexed" -gt 0 ]]; then
          printf 'warn: indexing job %s has %s indexed media item(s) but reported: %s\n' "$job_id" "$indexed" \
            "$(printf '%s' "$body" | json_eval 'data.failure?.message ?? "some files failed"')" >&2
          printf '%s' "$body" | json_eval '(data.logs ?? []).filter((log) => log.level === "Warn" || log.level === "Error").map((log) => "warn: " + log.message).filter((line, index, all) => all.indexOf(line) === index).join("\n")' >&2
          return 0
        fi
        printf 'error: indexing job %s finished with status %s:\n%s\n' "$job_id" "$status" "$body" >&2
        return 1
        ;;
      Cancelled)
        printf 'error: indexing job %s finished with status %s:\n%s\n' "$job_id" "$status" "$body" >&2
        return 1
        ;;
    esac
    if ((SECONDS >= deadline)); then
      printf 'error: indexing job %s did not finish within %ss (status %s)\n' "$job_id" "$index_timeout_seconds" "$status" >&2
      return 1
    fi
    sleep 2
  done
}

sample_query_file() {
  local preferred="$sample_query_dir/wikimedia-example-query.jpg"
  if [[ -f "$preferred" ]]; then
    printf '%s\n' "$preferred"
    return
  fi
  find "$sample_query_dir" -type f 2>/dev/null | sort | head -n 1
}

# Set by seed_showcase for the full flow's summary line.
seed_query_file=""
seed_result_count=0
seed_top_match=""

seed_showcase() {
  local tmp body_file job_id code
  seed_query_file="$(sample_query_file)"
  if [[ -z "$seed_query_file" ]]; then
    printf 'error: no sample query files in %s; run bun run sample:download\n' "$sample_query_dir" >&2
    return 1
  fi

  wait_for_backend

  tmp="$(mktemp -d)"
  body_file="$tmp/body.json"
  # shellcheck disable=SC2064
  trap "rm -rf '$tmp'; trap - RETURN" RETURN

  job_id="$(start_index_job "$body_file")"
  if [[ -z "$job_id" ]]; then
    printf 'error: %s did not return an indexing job id\n' "$api_url" >&2
    return 1
  fi
  wait_for_index_job "$job_id"

  code="$(curl -sS --max-time 300 -o "$body_file" -w '%{http_code}' -X POST \
    -F "file=@${seed_query_file}" "$api_url/api/search?limit=12")" || {
    printf 'error: sample query against %s/api/search failed\n' "$api_url" >&2
    return 1
  }
  if [[ "$code" != 2?? ]]; then
    printf 'error: POST %s/api/search returned HTTP %s:\n' "$api_url" "$code" >&2
    cat "$body_file" >&2
    printf '\n' >&2
    return 1
  fi

  seed_result_count="$(json_eval '(data.results ?? []).length + (data.scenes ?? []).reduce((total, scene) => total + (scene.results ?? []).length, 0)' <"$body_file")"
  seed_top_match="$(json_eval '((data.results ?? [])[0] ?? (data.scenes ?? []).flatMap((scene) => scene.results ?? [])[0])?.image?.filename ?? ""' <"$body_file")"

  printf 'sample query returned %s result(s) for %s\n' "$seed_result_count" "${seed_query_file#"$project_root"/}"
  if [[ "$seed_result_count" -lt 1 ]]; then
    printf 'error: the sample query returned no results; check the indexing job logs at %s/api/jobs/%s\n' "$api_url" "$job_id" >&2
    return 1
  fi
}

# --- full flow -----------------------------------------------------------------------

showcase_app_dir="$(resolve_path "${SHOWCASE_APP_DATA_DIR:-.dev-data/showcase}")"
vite_pid=""
started_qdrant=0

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  # The reader of our output may already be gone (for example a test runner that sent
  # SIGINT and exited); keep cleaning up instead of dying on SIGPIPE.
  trap '' PIPE
  local pid
  for pid in "$vite_pid" "$backend_pid"; do
    [[ -n "$pid" ]] || continue
    kill -TERM "$pid" 2>/dev/null || true
  done
  for pid in "$vite_pid" "$backend_pid"; do
    [[ -n "$pid" ]] || continue
    wait "$pid" 2>/dev/null || true
  done
  if [[ "$started_qdrant" -eq 1 ]]; then
    printf 'Stopping the showcase Qdrant container\n' >&2 || true
    compose stop qdrant >/dev/null 2>&1 || true
  fi
  exit "$status"
}

export_backend_env() {
  export APP_DATA_DIR="$showcase_app_dir"
  export QDRANT_URL="$qdrant_url"
  export QDRANT_COLLECTION="${SHOWCASE_QDRANT_COLLECTION:-media_similarity_showcase}"
  export BIND_ADDR="127.0.0.1:8000"
  export FRONTEND_SERVING_ENABLED=false
  export MEDIA_SOURCES_FILE="$showcase_app_dir/media-sources.txt"
  export MEDIA_SOURCES_SEED_FILE=""
  export SOURCE_IMAGE_DIR="$sample_source_dir"
  export THUMBNAIL_DIR="$showcase_app_dir/thumbnails"
  export UPLOAD_DIR="$showcase_app_dir/uploads"
  export INDEXING_LEDGER_FILE="$showcase_app_dir/indexing-ledger.json"
  export PROCESSING_WORKFLOWS_FILE="$showcase_app_dir/processing-workflows.json"
  export VOICE_REGISTRY_PATH="$showcase_app_dir/recognized-voices.json"
  export SMART_ALBUMS_FILE="$showcase_app_dir/smart-albums.json"
  export MODEL_BUNDLE_DIR="$showcase_app_dir/models/bundles"
  export MODEL_HF_CACHE_DIR="$showcase_app_dir/models/hf-cache"
  # Heavy model roles stay off: search runs on pHash and the built-in color vectors,
  # and /api/ready does not wait for model downloads. Enable roles later in the UI.
  export VISUAL_EMBEDDING_ENABLED=false
  export FACE_ANALYSIS_ENABLED=false
  export AUDIO_TRANSCRIPTION_ENABLED=false
  export AUDIO_TRANSCRIPTION_AUTO_DOWNLOAD=false
  export OCR_ENABLED=false
  export STARTUP_INDEXING_ENABLED=false
  export SOURCE_WATCHING_ENABLED=false
  export VIDEO_MAX_FRAMES="${VIDEO_MAX_FRAMES:-120}"
  export MAX_UPLOAD_MB="${MAX_UPLOAD_MB:-100}"
}

write_media_sources() {
  mkdir -p "$showcase_app_dir"
  {
    printf '# Managed by bun run showcase; rewritten on every run.\n'
    printf '%s\n' "$sample_source_dir"
  } >"$showcase_app_dir/media-sources.txt"
}

ensure_ports_free() {
  if curl -fsS --max-time 2 "http://127.0.0.1:8000/api/health" >/dev/null 2>&1; then
    printf 'error: a backend already listens on 127.0.0.1:8000 (bun dev?); stop it before bun run showcase\n' >&2
    return 1
  fi
  if curl -sS --max-time 2 -o /dev/null "$ui_url" 2>/dev/null; then
    printf 'error: port %s is already in use (bun dev?); stop it before bun run showcase\n' "$ui_port" >&2
    return 1
  fi
}

start_qdrant() {
  if curl -fsS --max-time 2 "$qdrant_url/readyz" >/dev/null 2>&1; then
    printf 'Reusing the Qdrant instance at %s\n' "$qdrant_url" >&2
    return
  fi
  printf 'Starting Qdrant with %s\n' "${COMPOSE_CMD[*]}" >&2
  started_qdrant=1
  compose up -d qdrant
  local deadline=$((SECONDS + 120))
  until curl -fsS --max-time 2 "$qdrant_url/readyz" >/dev/null 2>&1; do
    if ((SECONDS >= deadline)); then
      printf 'error: Qdrant did not become ready at %s; last container logs:\n' "$qdrant_url" >&2
      compose logs --tail 20 qdrant >&2 || true
      return 1
    fi
    sleep 1
  done
}

wait_for_ui() {
  local deadline=$((SECONDS + wait_seconds))
  until curl -fsS --max-time 2 -o /dev/null "$ui_url"; do
    if ! kill -0 "$vite_pid" 2>/dev/null; then
      printf 'error: the Vite UI exited before %s became reachable\n' "$ui_url" >&2
      return 1
    fi
    if ((SECONDS >= deadline)); then
      printf 'error: UI at %s is not reachable after %ss\n' "$ui_url" "$wait_seconds" >&2
      return 1
    fi
    sleep 1
  done
}

run_showcase() {
  check_prereqs || exit 1
  ensure_ports_free

  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM

  printf 'Building the backend (first run can take several minutes)\n' >&2
  cargo build --manifest-path backend/Cargo.toml --bin sample_corpus --bin image-similarity-service
  local target_dir
  target_dir="$(cargo metadata --format-version 1 --no-deps --manifest-path backend/Cargo.toml | json_eval 'data.target_directory')"

  if [[ ! -d "$sample_source_dir" || -z "$(sample_query_file)" ]]; then
    printf 'Downloading the sample corpus to %s\n' "$sample_root" >&2
    "$target_dir/debug/sample_corpus" download --output "$sample_root"
  fi

  APP_DATA_DIR="$showcase_app_dir" bash scripts/prepare-dev-data.sh
  write_media_sources
  start_qdrant
  export_backend_env

  # The backend runs as its built binary (not through cargo run) and `bun x` forwards
  # signals to Vite, so stopping these two PIDs stops every child. They stay in the
  # caller's process group, so Ctrl-C reaches them and this script alike.
  "$target_dir/debug/image-similarity-service" &
  backend_pid="$!"
  bun x vite --host 127.0.0.1 --port "$ui_port" --strictPort &
  vite_pid="$!"

  seed_showcase
  wait_for_ui

  printf 'sample query: upload %s in the UI (it returned %s result(s), top match %s)\nshowcase ready: %s\n' \
    "${seed_query_file#"$project_root"/}" "$seed_result_count" "${seed_top_match:-n/a}" "$ui_url"
  printf 'Press Ctrl-C to stop the showcase.\n' >&2

  wait -n "$backend_pid" "$vite_pid"
}

case "${1:-}" in
  "")
    run_showcase
    ;;
  --check-prereqs)
    check_prereqs
    ;;
  --seed-only)
    seed_showcase
    ;;
  --check)
    check_showcase
    ;;
  --dev)
    check_showcase
    bun dev
    ;;
  -h | --help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

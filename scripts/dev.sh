#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

resolve_path() {
  local path="$1"
  case "$path" in
    /*)
      printf '%s\n' "$path"
      ;;
    *)
      printf '%s/%s\n' "$project_root" "$path"
      ;;
  esac
}

seed_local_media_sources() {
  local target="$1"
  if [[ -e "$target" ]]; then
    return
  fi

  mkdir -p "$(dirname "$target")"
  {
    printf '# Managed by bun dev.\n'
    printf '# One local path or source URI per line.\n'
    printf '%s\n' "${HOST_PICTURES_DIR:-${HOME}/Pictures}"
    printf '%s\n' "${HOST_VIDEO_DIR:-${HOME}/Videos}"
    printf '%s\n' "${HOST_AUDIO_DIR:-${HOME}/Music}"
  } >"$target"
}

bash scripts/prepare-dev-data.sh
HOST_UID="$(id -u)" HOST_GID="$(id -g)" docker compose up -d qdrant

app_data_dir="$(resolve_path "${APP_DATA_DIR:-.dev-data/app}")"
media_sources_file="${MEDIA_SOURCES_FILE:-${app_data_dir}/media-sources.txt}"
seed_local_media_sources "$media_sources_file"

export QDRANT_URL="${QDRANT_URL:-http://127.0.0.1:6333}"
export BIND_ADDR="${BIND_ADDR:-127.0.0.1:8000}"
export FRONTEND_SERVING_ENABLED="${FRONTEND_SERVING_ENABLED:-false}"
export MEDIA_SOURCES_FILE="$media_sources_file"
export MEDIA_SOURCES_SEED_FILE="${MEDIA_SOURCES_SEED_FILE:-}"
export SOURCE_IMAGE_DIR="${SOURCE_IMAGE_DIR:-${HOST_PICTURES_DIR:-${HOME}/Pictures}}"
export THUMBNAIL_DIR="${THUMBNAIL_DIR:-${app_data_dir}/thumbnails}"
export UPLOAD_DIR="${UPLOAD_DIR:-${app_data_dir}/uploads}"
export INDEXING_LEDGER_FILE="${INDEXING_LEDGER_FILE:-${app_data_dir}/indexing-ledger.json}"
export PROCESSING_WORKFLOWS_FILE="${PROCESSING_WORKFLOWS_FILE:-${app_data_dir}/processing-workflows.json}"
export VOICE_REGISTRY_PATH="${VOICE_REGISTRY_PATH:-${app_data_dir}/recognized-voices.json}"
export SMART_ALBUMS_FILE="${SMART_ALBUMS_FILE:-${app_data_dir}/smart-albums.json}"
export MODEL_BUNDLE_DIR="${MODEL_BUNDLE_DIR:-${app_data_dir}/models/bundles}"
export MODEL_HF_CACHE_DIR="${MODEL_HF_CACHE_DIR:-${app_data_dir}/models/hf-cache}"

cargo run --manifest-path backend/Cargo.toml --bin image-similarity-service &
api_pid="$!"

bunx vite --host 127.0.0.1 &
vite_pid="$!"

cleanup() {
  kill "$api_pid" "$vite_pid" 2>/dev/null || true
  wait "$api_pid" "$vite_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

wait -n "$api_pid" "$vite_pid"

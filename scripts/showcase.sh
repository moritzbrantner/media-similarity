#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

sample_source_dir="$project_root/sample-images/showcase/sources"
sample_query_dir="$project_root/sample-images/showcase/queries"

usage() {
  cat <<'USAGE'
Usage: bash scripts/showcase.sh --check|--dev

Options:
  --check  Validate local prerequisites and print sample-corpus .env values.
  --dev    Run --check, then start the regular development stack with bun dev.
USAGE
}

require_command() {
  local command_name="$1"
  if command -v "$command_name" >/dev/null 2>&1; then
    printf 'ok: %s is available\n' "$command_name"
  else
    printf 'missing: %s is not on PATH\n' "$command_name" >&2
    return 1
  fi
}

warn_command() {
  local command_name="$1"
  if command -v "$command_name" >/dev/null 2>&1; then
    printf 'ok: %s is available\n' "$command_name"
  else
    printf 'warn: %s is not on PATH; related media workflows may be unavailable\n' "$command_name" >&2
  fi
}

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

  require_command bun || failed=1
  require_command cargo || failed=1
  require_command docker || failed=1
  warn_command ffmpeg
  warn_command ffprobe
  warn_command pdfinfo
  warn_command pdftoppm
  warn_command pdftotext

  if [[ -d ../rust-packages ]]; then
    printf 'ok: ../rust-packages is present\n'
  else
    printf 'missing: ../rust-packages is required for Rust path dependencies\n' >&2
    failed=1
  fi

  cargo run --manifest-path backend/Cargo.toml --bin sample_corpus -- check

  if [[ -d "$sample_source_dir" && -d "$sample_query_dir" ]]; then
    printf 'ok: sample showcase files are present\n'
  else
    printf 'warn: sample showcase files are not downloaded yet; run bun run showcase:download\n' >&2
  fi

  print_env_values

  return "$failed"
}

case "${1:-}" in
  --check)
    check_showcase
    ;;
  --dev)
    check_showcase
    bun dev
    ;;
  -h|--help|"")
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

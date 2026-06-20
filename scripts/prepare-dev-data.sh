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

host_uid="${HOST_UID:-$(id -u)}"
host_gid="${HOST_GID:-$(id -g)}"

export HOST_UID="$host_uid"
export HOST_GID="$host_gid"

paths=(
  "$(resolve_path "${APP_DATA_DIR:-.dev-data/app}")"
  "$(resolve_path "${QDRANT_DATA_DIR:-.dev-data/qdrant}")"
  "$(resolve_path "${QDRANT_SNAPSHOTS_DIR:-.dev-data/qdrant-snapshots}")"
)

for path in "${paths[@]}"; do
  mkdir -p "$path"
  chmod -R u+rwX "$path" 2>/dev/null || true

  if [[ ! -w "$path" ]]; then
    unwritable_path="$path"
  else
    unwritable_path="$(find "$path" ! -writable -print -quit 2>/dev/null || true)"
  fi

  if [[ -n "$unwritable_path" ]]; then
    printf 'Data path is not writable: %s\n' "$unwritable_path" >&2
    printf 'Stop the compose services, then fix ownership with:\n' >&2
    printf '  sudo chown -R %s:%s %s\n' "$host_uid" "$host_gid" "$path" >&2
    exit 1
  fi
done

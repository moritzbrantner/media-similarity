#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

section() {
  printf '
== %s ==
' "$1"
}

section "Git status"
git status --short -- . || true

section "Branch"
git branch --show-current || true

section "Upstream"
upstream="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
if [[ -n "$upstream" ]]; then
  printf '%s
' "$upstream"
else
  printf 'none
'
fi

section "Ahead/behind"
if [[ -n "$upstream" ]]; then
  git rev-list --left-right --count "${upstream}...HEAD" || true
else
  printf 'skipped; no upstream branch
'
fi

section "Compose services"
if [[ -f docker-compose.yml || -f docker-compose.yaml || -f compose.yml || -f compose.yaml ]]; then
  docker compose ps || true
else
  printf 'no compose file at project root
'
fi

section "Running containers for image-similarity-service"
docker ps --filter "label=com.docker.compose.project=image-similarity-service" --format 'table {{.Names}}	{{.Status}}	{{.Ports}}' || true

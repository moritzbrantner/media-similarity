#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

qdrant_image="${QUALITY_QDRANT_IMAGE:-qdrant/qdrant:v1.12.5}"
quality_image="${QUALITY_RUNTIME_IMAGE:-media-similarity-quality:local}"
qdrant_port="${QUALITY_QDRANT_PORT:-6336}"
qdrant_grpc_port="${QUALITY_QDRANT_GRPC_PORT:-6337}"
qdrant_url="http://127.0.0.1:${qdrant_port}"
qdrant_container="media-sim-quality-qdrant-$$"
qdrant_data_dir="$(mktemp -d)"
qdrant_snapshots_dir="$(mktemp -d)"
quality_model_root="${QUALITY_MODEL_ROOT:-/tmp/media-similarity-quality-models}"
if [[ -n "${MODEL_BUNDLE_DIR:-}" ]]; then
  model_bundle_dir="$MODEL_BUNDLE_DIR"
  model_hf_cache_dir="${MODEL_HF_CACHE_DIR:-${project_root}/.dev-data/app/models/hf-cache}"
elif [[ -d "${quality_model_root}/bundles" ]]; then
  model_bundle_dir="${quality_model_root}/bundles"
  model_hf_cache_dir="${quality_model_root}/hf-cache"
else
  model_bundle_dir="${project_root}/.dev-data/app/models/bundles"
  model_hf_cache_dir="${project_root}/.dev-data/app/models/hf-cache"
fi
keep_artifacts=0

for arg in "$@"; do
  if [[ "$arg" == "--keep-artifacts" ]]; then
    keep_artifacts=1
    break
  fi
done

cleanup() {
  local status=$?
  trap - EXIT INT TERM

  docker stop "$qdrant_container" >/dev/null 2>&1 || true
  docker rm -f "$qdrant_container" >/dev/null 2>&1 || true

  if [[ "$status" -eq 0 && "$keep_artifacts" -eq 0 ]]; then
    rm -rf "$qdrant_data_dir" "$qdrant_snapshots_dir"
  else
    {
      printf 'Preserving quality Qdrant data dir: %s\n' "$qdrant_data_dir"
      printf 'Preserving quality Qdrant snapshots dir: %s\n' "$qdrant_snapshots_dir"
    } >&2
  fi

  exit "$status"
}

wait_for_qdrant() {
  local ready_url="${qdrant_url}/readyz"
  local deadline=$((SECONDS + 45))

  printf 'Waiting for Qdrant at %s\n' "$ready_url" >&2
  until curl --noproxy '*' -fsS --max-time 5 "$ready_url" >/dev/null 2>&1; do
    if ((SECONDS >= deadline)); then
      printf 'Timed out waiting for Qdrant at %s\n' "$ready_url" >&2
      docker logs --tail=120 "$qdrant_container" >&2 || true
      return 1
    fi
    sleep 1
  done
}

if curl --noproxy '*' -fsS --max-time 1 "${qdrant_url}/readyz" >/dev/null 2>&1; then
  printf 'Refusing to start disposable Qdrant: %s is already serving /readyz\n' "$qdrant_url" >&2
  printf 'Set QUALITY_QDRANT_PORT to an unused port and retry.\n' >&2
  exit 1
fi

trap cleanup EXIT INT TERM

mkdir -p "$model_bundle_dir" "$model_hf_cache_dir"

docker run --rm -d \
  --name "$qdrant_container" \
  --network host \
  --user "$(id -u):$(id -g)" \
  -e QDRANT__SERVICE__HTTP_PORT="$qdrant_port" \
  -e QDRANT__SERVICE__GRPC_PORT="$qdrant_grpc_port" \
  -v "${qdrant_data_dir}:/qdrant/storage" \
  -v "${qdrant_snapshots_dir}:/qdrant/snapshots" \
  "$qdrant_image" >/dev/null

wait_for_qdrant

if [[ "${QUALITY_SKIP_IMAGE_BUILD:-}" != "1" ]]; then
  build_args=()
  if [[ "${QUALITY_NO_CACHE:-}" == "1" ]]; then
    build_args+=(--no-cache)
  fi
  docker buildx build \
    --load \
    "${build_args[@]}" \
    --build-context rust-packages=../rust-packages \
    --target api-runtime \
    -t "$quality_image" \
    .
fi

docker run --rm \
  --user "$(id -u):$(id -g)" \
  -e HOME=/tmp \
  -e QUALITY_REPO_ROOT=/workspace/image-similarity-service \
  -e MODEL_BUNDLE_DIR=/workspace/image-similarity-service/.dev-data/app/models/bundles \
  -e MODEL_HF_CACHE_DIR=/workspace/image-similarity-service/.dev-data/app/models/hf-cache \
  -e MODEL_HF_TOKEN="${MODEL_HF_TOKEN:-}" \
  -v "${project_root}:/workspace/image-similarity-service" \
  -v "${model_bundle_dir}:/workspace/image-similarity-service/.dev-data/app/models/bundles" \
  -v "${model_hf_cache_dir}:/workspace/image-similarity-service/.dev-data/app/models/hf-cache" \
  -w /workspace/image-similarity-service \
  "$quality_image" \
  quality_corpus download-models

docker run --rm \
  --network host \
  --user "$(id -u):$(id -g)" \
  -e HOME=/tmp \
  -e QUALITY_REPO_ROOT=/workspace/image-similarity-service \
  -e QDRANT_URL="$qdrant_url" \
  -e MODEL_BUNDLE_DIR=/workspace/image-similarity-service/.dev-data/app/models/bundles \
  -e MODEL_HF_CACHE_DIR=/workspace/image-similarity-service/.dev-data/app/models/hf-cache \
  -e MODEL_HF_TOKEN="${MODEL_HF_TOKEN:-}" \
  -v "${project_root}:/workspace/image-similarity-service" \
  -v "${model_bundle_dir}:/workspace/image-similarity-service/.dev-data/app/models/bundles" \
  -v "${model_hf_cache_dir}:/workspace/image-similarity-service/.dev-data/app/models/hf-cache" \
  -w /workspace/image-similarity-service \
  "$quality_image" \
  quality_corpus evaluate "$@"

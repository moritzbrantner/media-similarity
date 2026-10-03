#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/service-smoke.sh [--disposable] [--sample-corpus]

Runs a Docker Compose service-mode smoke check for api, web, and qdrant.

By default this uses the normal persisted Compose data directories and stops
only services started by the check. Pass --disposable to use temporary app and
Qdrant data directories that are removed after the check.

Pass --sample-corpus to validate service startup, readiness, indexing, search,
and generated artifact serving against sample-images/showcase.
USAGE
}

disposable=0
sample_corpus=0
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --disposable)
      disposable=1
      shift
      ;;
    --sample-corpus)
      sample_corpus=1
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

sample_root="$project_root/sample-images/showcase"
sample_source_dir="$sample_root/sources"
sample_query_dir="$sample_root/queries"

if [[ "$sample_corpus" -eq 1 ]]; then
  if ! command -v bun >/dev/null 2>&1; then
    printf 'Sample-corpus smoke checks require bun on PATH.\n' >&2
    exit 69
  fi
  if [[ ! -d "$sample_source_dir" || ! -d "$sample_query_dir" ]]; then
    printf 'Sample corpus is not downloaded at %s.\n' "$sample_root" >&2
    printf 'Run `bun run showcase:download` before `--sample-corpus`.\n' >&2
    exit 66
  fi
  bun run sample:check
fi

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

if [[ "$sample_corpus" -eq 1 ]]; then
  export HOST_PICTURES_DIR="$sample_source_dir"
  export HOST_VIDEO_DIR="$sample_source_dir"
  export HOST_AUDIO_DIR="$sample_source_dir"
  export MAX_UPLOAD_MB="${MAX_UPLOAD_MB:-100}"
fi

web_port="${WEB_PORT:-5173}"
api_port="${API_PORT:-8000}"
web_url="http://127.0.0.1:${web_port}"
proxied_health_url="${web_url}/api/health"
direct_health_url="http://127.0.0.1:${api_port}/api/health"
direct_api_url="http://127.0.0.1:${api_port}"
direct_ready_url="${direct_api_url}/api/ready"

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

json_eval() {
  local expression="$1"
  JSON_EXPR="$expression" bun --eval '
const fs = require("fs");
const input = fs.readFileSync(0, "utf8");
const data = input.trim() ? JSON.parse(input) : null;
const result = Function("data", "return (" + process.env.JSON_EXPR + ");")(data);
if (Array.isArray(result)) {
  for (const value of result) console.log(value);
} else if (result !== undefined && result !== null) {
  console.log(String(result));
}
'
}

wait_for_job() {
  local job_id="$1"
  local deadline=$((SECONDS + 600))
  local body status

  printf 'Waiting for job %s\n' "$job_id" >&2
  while true; do
    body="$(curl -fsS "${direct_api_url}/api/jobs/${job_id}")"
    status="$(printf '%s' "$body" | json_eval 'data.status')"
    case "$status" in
      Succeeded)
        return 0
        ;;
      Failed | Cancelled)
        printf 'Job %s finished with status %s:\n%s\n' "$job_id" "$status" "$body" >&2
        return 1
        ;;
    esac

    if ((SECONDS >= deadline)); then
      printf 'Timed out waiting for job %s. Last status: %s\n%s\n' "$job_id" "$status" "$body" >&2
      return 1
    fi
    sleep 2
  done
}

download_required_models_once() {
  local readiness roles role response job_id downloaded=0
  readiness="$(curl -sS "$direct_ready_url" || true)"
  mapfile -t roles < <(printf '%s' "$readiness" | json_eval '
(data?.checks ?? [])
  .filter((check) => check.status === "error" && String(check.name ?? "").startsWith("model."))
  .map((check) => String(check.name).replace(/^model\./, ""))
')

  for role in "${roles[@]}"; do
    [[ -n "$role" ]] || continue
    printf 'Downloading required model role %s\n' "$role" >&2
    response="$(curl -fsS -X POST \
      -H 'Content-Type: application/json' \
      -d '{"model":null}' \
      "${direct_api_url}/api/models/${role}/download")"
    job_id="$(printf '%s' "$response" | json_eval 'data.spec.id')"
    wait_for_job "$job_id"
    downloaded=1
  done

  return "$downloaded"
}

provision_required_models() {
  local attempt
  for attempt in 1 2 3; do
    if download_required_models_once; then
      return 0
    fi
    printf 'Re-checking readiness after model provisioning pass %s\n' "$attempt" >&2
  done
  return 0
}

wait_for_ready() {
  local deadline=$((SECONDS + 120))
  local body

  printf 'Waiting for backend readiness at %s\n' "$direct_ready_url" >&2
  until curl -fsS "$direct_ready_url" >/dev/null; do
    if ((SECONDS >= deadline)); then
      body="$(curl -sS "$direct_ready_url" || true)"
      printf 'Timed out waiting for readiness at %s\n%s\n' "$direct_ready_url" "$body" >&2
      docker compose ps api web qdrant >&2 || true
      return 1
    fi
    sleep 2
  done
}

sample_search_cases() {
  bun --eval '
const path = require("path");
const fs = require("fs");
const manifest = JSON.parse(fs.readFileSync("tests/fixtures/sample-corpus/manifest.json", "utf8"));
const assets = new Map(manifest.assets.map((asset) => [asset.id, asset]));
for (const search of manifest.searches) {
  const query = assets.get(search.query_asset);
  const expected = assets.get(search.expected_top_match);
  if (!query || !expected) throw new Error(`Invalid search case ${search.id}`);
  console.log([
    search.id,
    query.kind,
    query.filename,
    path.basename(expected.filename),
  ].join("\t"));
}
'
}

assert_index_success() {
  json_eval '
(() => {
  if (data.failed !== 0) {
    throw new Error(`indexing failed for ${data.failed} item(s): ${JSON.stringify(data.errors ?? [])}`);
  }
  if ((data.indexed ?? 0) + (data.already_indexed ?? 0) <= 0) {
    throw new Error(`indexing did not produce indexed or already-indexed media: ${JSON.stringify(data)}`);
  }
  return "ok";
})()
' >/dev/null
}

assert_expected_top_match() {
  local search_id="$1"
  local expected_basename="$2"
  json_eval "
(() => {
  const sceneResults = (data.scenes ?? []).flatMap((scene) => scene.results ?? []);
  const first = (data.results ?? [])[0] ?? sceneResults[0];
  if (!first?.image) {
    throw new Error('${search_id} returned no results');
  }
  const image = first.image;
  const haystack = [
    image.filename,
    image.relative_path,
    image.path,
    image.source_uri,
    image.source_item_uri,
    image.full_video_url,
    image.full_audio_url,
    image.full_pdf_url,
    image.pdf_page_url,
    image.scene_clip_url,
  ].filter(Boolean).join('\\n');
  if (!haystack.includes('${expected_basename}')) {
    throw new Error('${search_id} expected top match ${expected_basename}, got ' + haystack);
  }
  return 'ok';
})()
" >/dev/null
}

response_artifact_urls() {
  json_eval '
(() => {
  const urls = new Set();
  const collectImage = (image) => {
    if (!image) return;
    for (const key of [
      "thumbnail_url",
      "animated_thumbnail_url",
      "full_video_url",
      "full_audio_url",
      "full_pdf_url",
      "pdf_page_url",
      "scene_clip_url",
    ]) {
      if (image[key]) urls.add(image[key]);
    }
    for (const artifact of image.artifacts ?? []) {
      if (artifact.url) urls.add(artifact.url);
    }
  };
  for (const result of data.results ?? []) collectImage(result.image);
  for (const scene of data.scenes ?? []) {
    if (scene.clip_url) urls.add(scene.clip_url);
    for (const result of scene.results ?? []) collectImage(result.image);
  }
  return [...urls].filter((url) => String(url).startsWith("/"));
})()
'
}

assert_artifact_urls() {
  local search_id="$1"
  local response="$2"
  local url

  while IFS= read -r url; do
    [[ -n "$url" ]] || continue
    printf 'Checking %s artifact %s\n' "$search_id" "$url" >&2
    curl -fsS "${direct_api_url}${url}" >/dev/null
  done < <(printf '%s' "$response" | response_artifact_urls)
}

run_sample_corpus_checks() {
  local index_response search_id kind query_filename expected_basename query_path response

  wait_for_http "web UI" "$web_url"
  wait_for_http "proxied backend health" "$proxied_health_url"
  wait_for_http "direct backend health" "$direct_health_url"
  provision_required_models
  wait_for_ready

  printf 'Indexing sample corpus sources\n' >&2
  index_response="$(curl -fsS -X POST "${direct_api_url}/api/index")"
  printf '%s' "$index_response" | assert_index_success

  while IFS=$'\t' read -r search_id kind query_filename expected_basename; do
    query_path="$sample_root/$query_filename"
    if [[ ! -f "$query_path" ]]; then
      printf 'Missing query file for %s: %s\n' "$search_id" "$query_path" >&2
      return 1
    fi
    printf 'Searching sample case %s (%s)\n' "$search_id" "$kind" >&2
    response="$(curl -fsS -X POST "${direct_api_url}/api/search?limit=12" -F "file=@${query_path}")"
    printf '%s' "$response" | assert_expected_top_match "$search_id" "$expected_basename"
    assert_artifact_urls "$search_id" "$response"
  done < <(sample_search_cases)

  printf 'Sample-corpus service smoke check passed: %s\n' "$web_url"
}

docker compose config >/dev/null

export COMPOSE_SERVICES="${COMPOSE_SERVICES:-api web qdrant}"
export SERVICE_SMOKE_WEB_URL="$web_url"
export SERVICE_SMOKE_PROXIED_HEALTH_URL="$proxied_health_url"
export SERVICE_SMOKE_DIRECT_HEALTH_URL="$direct_health_url"
export SERVICE_SMOKE_DIRECT_API_URL="$direct_api_url"
export SERVICE_SMOKE_DIRECT_READY_URL="$direct_ready_url"
export SERVICE_SMOKE_SAMPLE_ROOT="$sample_root"

if [[ "$sample_corpus" -eq 1 ]]; then
  bash scripts/run-with-compose-services.sh bash -c "$(declare -f wait_for_http json_eval wait_for_job download_required_models_once provision_required_models wait_for_ready sample_search_cases assert_index_success assert_expected_top_match response_artifact_urls assert_artifact_urls run_sample_corpus_checks); web_url=\"\$SERVICE_SMOKE_WEB_URL\"; proxied_health_url=\"\$SERVICE_SMOKE_PROXIED_HEALTH_URL\"; direct_health_url=\"\$SERVICE_SMOKE_DIRECT_HEALTH_URL\"; direct_api_url=\"\$SERVICE_SMOKE_DIRECT_API_URL\"; direct_ready_url=\"\$SERVICE_SMOKE_DIRECT_READY_URL\"; sample_root=\"\$SERVICE_SMOKE_SAMPLE_ROOT\"; run_sample_corpus_checks"
else
  bash scripts/run-with-compose-services.sh bash -c "$(declare -f wait_for_http run_smoke_checks); web_url=\"\$SERVICE_SMOKE_WEB_URL\"; proxied_health_url=\"\$SERVICE_SMOKE_PROXIED_HEALTH_URL\"; direct_health_url=\"\$SERVICE_SMOKE_DIRECT_HEALTH_URL\"; run_smoke_checks"
fi

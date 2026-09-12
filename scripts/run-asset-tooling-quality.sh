#!/usr/bin/env bash
set -uo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

output_host="${project_root}/sample-images/quality"
compatibility_host="${output_host}/.asset-tooling-evaluator-root"
compatibility_container="/workspace/image-similarity-service/sample-images/quality/.asset-tooling-evaluator-root"
output_container="/workspace/image-similarity-service/sample-images/quality"
raw_report_host="${compatibility_host}/benchmarks/results/quality-corpus-report.json"
relationships="tests/fixtures/quality-corpus/asset-tooling/relationships.json"
results_dir="${project_root}/benchmarks/results"

if [[ ! -f "${output_host}/asset-tooling-materialization.json" || ! -f "${compatibility_host}/tests/fixtures/quality-corpus/manifest.json" ]]; then
  printf 'Asset-tooling quality corpus is not materialized. Run `bun run quality:asset-tooling:prepare` first.\n' >&2
  exit 2
fi

QUALITY_EVALUATION_REPO_ROOT="$compatibility_container" \
  bash scripts/run-quality-gate-once.sh \
    --output "$output_container" \
    --skip-download-check \
    "$@"
evaluator_status=$?

metrics_status=0
if [[ -f "$raw_report_host" ]]; then
  mkdir -p "$results_dir"
  cp "$raw_report_host" "${results_dir}/asset-tooling-quality-raw-report.json"
  bun scripts/quality-metrics.mjs \
    --report "$raw_report_host" \
    --relationships "$relationships" \
    --output-dir "$results_dir" || metrics_status=$?
else
  printf 'Quality evaluator did not produce %s\n' "$raw_report_host" >&2
  metrics_status=3
fi

if [[ "$evaluator_status" -ne 0 ]]; then
  exit "$evaluator_status"
fi
exit "$metrics_status"

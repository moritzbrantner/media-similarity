import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { evaluateQualityMetrics, qualityMetricsMarkdown } from "./quality-metrics.mjs";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const reportIndex = process.argv.indexOf("--report");
if (reportIndex < 0 || !process.argv[reportIndex + 1]) {
  throw new Error("usage: bun scripts/quality-audio-metrics.mjs --report PATH [--output-dir PATH]");
}
const outputIndex = process.argv.indexOf("--output-dir");
const outputDir = outputIndex >= 0
  ? path.resolve(repoRoot, process.argv[outputIndex + 1])
  : path.join(repoRoot, "benchmarks/results");
const report = JSON.parse(await readFile(path.resolve(repoRoot, process.argv[reportIndex + 1]), "utf8"));
const relationships = JSON.parse(
  await readFile(path.join(repoRoot, "tests/fixtures/quality-corpus/asset-tooling/audio-relationships.json"), "utf8"),
);
const summary = evaluateQualityMetrics(report, relationships);
const markdown = qualityMetricsMarkdown(summary).replace(
  "# Asset-tooling quality metrics",
  "# Asset-tooling audio quality metrics",
);
await mkdir(outputDir, { recursive: true });
await writeFile(path.join(outputDir, "asset-tooling-audio-quality-report.json"), `${JSON.stringify(summary, null, 2)}\n`);
await writeFile(path.join(outputDir, "asset-tooling-audio-quality-report.md"), markdown);
process.stdout.write(`${markdown}\n`);

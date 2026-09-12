import { readFile, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

function finiteNumber(value, location) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${location} must be a finite number`);
  }
  return value;
}

function average(values) {
  return values.length === 0 ? 0 : values.reduce((sum, value) => sum + value, 0) / values.length;
}

function dcg(grades, k) {
  let value = 0;
  for (let index = 0; index < Math.min(k, grades.length); index += 1) {
    const grade = grades[index] ?? 0;
    value += (2 ** grade - 1) / Math.log2(index + 2);
  }
  return value;
}

export function validateQualityRelationships(value) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("quality relationships must be an object");
  }
  if (value.version !== 1) throw new Error("quality relationships version must be 1");
  if (!Array.isArray(value.searches) || value.searches.length === 0) {
    throw new Error("quality relationships searches must be a non-empty array");
  }
  const recallK = value.metrics?.recallK;
  if (!Array.isArray(recallK) || recallK.length === 0 || recallK.some((k) => !Number.isSafeInteger(k) || k < 1)) {
    throw new Error("quality relationships metrics.recallK must contain positive integers");
  }
  const ndcgK = value.metrics?.ndcgK;
  const hardNegativeK = value.metrics?.hardNegativeK;
  if (!Number.isSafeInteger(ndcgK) || ndcgK < 1) throw new Error("metrics.ndcgK must be a positive integer");
  if (!Number.isSafeInteger(hardNegativeK) || hardNegativeK < 1) {
    throw new Error("metrics.hardNegativeK must be a positive integer");
  }

  const ids = new Set();
  const searches = value.searches.map((search, index) => {
    if (typeof search !== "object" || search === null || Array.isArray(search)) {
      throw new Error(`relationships searches[${index}] must be an object`);
    }
    for (const field of ["searchId", "family", "queryAssetId"]) {
      if (typeof search[field] !== "string" || search[field].trim().length === 0) {
        throw new Error(`relationships searches[${index}].${field} must be a non-empty string`);
      }
    }
    if (ids.has(search.searchId)) throw new Error(`duplicate relationship search '${search.searchId}'`);
    ids.add(search.searchId);
    if (!Array.isArray(search.relevance) || search.relevance.length === 0) {
      throw new Error(`relationships search '${search.searchId}' needs relevance labels`);
    }
    const relevance = search.relevance.map((entry, relevanceIndex) => {
      if (typeof entry?.assetId !== "string" || entry.assetId.length === 0) {
        throw new Error(`relationships search '${search.searchId}' relevance[${relevanceIndex}] needs assetId`);
      }
      if (!Number.isSafeInteger(entry.grade) || entry.grade < 1 || entry.grade > 3) {
        throw new Error(`relationships search '${search.searchId}' relevance[${relevanceIndex}] grade must be 1..3`);
      }
      return { assetId: entry.assetId, grade: entry.grade };
    });
    if (new Set(relevance.map((entry) => entry.assetId)).size !== relevance.length) {
      throw new Error(`relationships search '${search.searchId}' contains duplicate relevance assets`);
    }
    const hardNegatives = search.hardNegatives ?? [];
    if (!Array.isArray(hardNegatives) || hardNegatives.some((id) => typeof id !== "string" || id.length === 0)) {
      throw new Error(`relationships search '${search.searchId}' hardNegatives must be strings`);
    }
    const relevant = new Set(relevance.map((entry) => entry.assetId));
    for (const negative of hardNegatives) {
      if (relevant.has(negative)) {
        throw new Error(`relationships search '${search.searchId}' marks '${negative}' as relevant and hard-negative`);
      }
    }
    return {
      searchId: search.searchId,
      family: search.family,
      queryAssetId: search.queryAssetId,
      relevance,
      hardNegatives: [...new Set(hardNegatives)],
    };
  });

  return {
    version: 1,
    metrics: { recallK: [...new Set(recallK)].sort((a, b) => a - b), ndcgK, hardNegativeK },
    searches,
  };
}

function rankedAssetIds(searchReport) {
  return (searchReport.top_k ?? []).map((hit) => hit.asset_id).filter((value) => typeof value === "string");
}

export function evaluateQualityMetrics(report, relationshipsValue) {
  const relationships = validateQualityRelationships(relationshipsValue);
  if (typeof report !== "object" || report === null || !Array.isArray(report.searches)) {
    throw new Error("quality report must contain searches");
  }
  const reportsById = new Map(report.searches.map((search) => [search.id, search]));
  const cases = relationships.searches.map((relationship) => {
    const searchReport = reportsById.get(relationship.searchId);
    if (!searchReport) throw new Error(`quality report is missing search '${relationship.searchId}'`);
    const ranking = rankedAssetIds(searchReport);
    const gradeByAsset = new Map(relationship.relevance.map((entry) => [entry.assetId, entry.grade]));
    const relevantAssets = relationship.relevance.map((entry) => entry.assetId);
    const relevantSet = new Set(relevantAssets);
    const recall = Object.fromEntries(
      relationships.metrics.recallK.map((k) => {
        const retrieved = new Set(ranking.slice(0, k).filter((assetId) => relevantSet.has(assetId)));
        return [`at${k}`, retrieved.size / relevantAssets.length];
      }),
    );
    const firstRelevant = ranking.findIndex((assetId) => relevantSet.has(assetId));
    const reciprocalRank = firstRelevant < 0 ? 0 : 1 / (firstRelevant + 1);
    const rankedGrades = ranking.map((assetId) => gradeByAsset.get(assetId) ?? 0);
    const idealGrades = relationship.relevance.map((entry) => entry.grade).sort((a, b) => b - a);
    const idealDcg = dcg(idealGrades, relationships.metrics.ndcgK);
    const ndcg = idealDcg === 0 ? 0 : dcg(rankedGrades, relationships.metrics.ndcgK) / idealDcg;
    const hardNegativeSet = new Set(relationship.hardNegatives);
    const hardNegativeHits = ranking
      .slice(0, relationships.metrics.hardNegativeK)
      .filter((assetId) => hardNegativeSet.has(assetId));
    const hardNegativeFalsePositiveRate =
      relationship.hardNegatives.length === 0 ? 0 : new Set(hardNegativeHits).size / relationship.hardNegatives.length;

    return {
      searchId: relationship.searchId,
      family: relationship.family,
      queryAssetId: relationship.queryAssetId,
      ranking,
      recall,
      reciprocalRank,
      ndcg,
      hardNegativeHits: [...new Set(hardNegativeHits)],
      hardNegativeFalsePositiveRate,
      evaluatorPassed: searchReport.passed === true,
    };
  });

  const families = {};
  for (const family of [...new Set(cases.map((entry) => entry.family))].sort()) {
    const members = cases.filter((entry) => entry.family === family);
    families[family] = aggregateCases(members, relationships.metrics);
  }

  return {
    schemaVersion: 1,
    corpus: report.corpus ?? null,
    metrics: relationships.metrics,
    aggregate: aggregateCases(cases, relationships.metrics),
    families,
    cases,
  };
}

function aggregateCases(cases, metricConfig) {
  const recall = Object.fromEntries(
    metricConfig.recallK.map((k) => [`at${k}`, average(cases.map((entry) => finiteNumber(entry.recall[`at${k}`], `recall@${k}`)))]),
  );
  return {
    cases: cases.length,
    recall,
    mrr: average(cases.map((entry) => entry.reciprocalRank)),
    ndcg: average(cases.map((entry) => entry.ndcg)),
    hardNegativeFalsePositiveRate: average(cases.map((entry) => entry.hardNegativeFalsePositiveRate)),
    evaluatorPassRate: average(cases.map((entry) => (entry.evaluatorPassed ? 1 : 0))),
  };
}

export function qualityMetricsMarkdown(summary) {
  const percent = (value) => `${(value * 100).toFixed(1)}%`;
  const lines = [
    "# Asset-tooling quality metrics",
    "",
    `Corpus: \`${summary.corpus ?? "unknown"}\``,
    "",
    "## Aggregate",
    "",
    `- Recall@1: ${percent(summary.aggregate.recall.at1 ?? 0)}`,
    `- Recall@5: ${percent(summary.aggregate.recall.at5 ?? 0)}`,
    `- MRR: ${summary.aggregate.mrr.toFixed(4)}`,
    `- nDCG@${summary.metrics.ndcgK}: ${summary.aggregate.ndcg.toFixed(4)}`,
    `- Hard-negative false-positive rate @${summary.metrics.hardNegativeK}: ${percent(summary.aggregate.hardNegativeFalsePositiveRate)}`,
    `- Existing evaluator pass rate: ${percent(summary.aggregate.evaluatorPassRate)}`,
    "",
    "## Families",
    "",
  ];
  for (const [family, metrics] of Object.entries(summary.families)) {
    lines.push(
      `- **${family}** (${metrics.cases}): R@1 ${percent(metrics.recall.at1 ?? 0)}, R@5 ${percent(metrics.recall.at5 ?? 0)}, MRR ${metrics.mrr.toFixed(4)}, nDCG ${metrics.ndcg.toFixed(4)}, hard-negative FP ${percent(metrics.hardNegativeFalsePositiveRate)}`,
    );
  }
  lines.push("", "## Cases", "");
  for (const entry of summary.cases) {
    lines.push(
      `- \`${entry.searchId}\`: R@1 ${percent(entry.recall.at1 ?? 0)}, R@5 ${percent(entry.recall.at5 ?? 0)}, RR ${entry.reciprocalRank.toFixed(4)}, nDCG ${entry.ndcg.toFixed(4)}, hard negatives [${entry.hardNegativeHits.join(", ") || "none"}]`,
    );
  }
  lines.push("");
  return lines.join("\n");
}

async function cli() {
  const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
  const args = process.argv.slice(2);
  const reportIndex = args.indexOf("--report");
  const relationshipsIndex = args.indexOf("--relationships");
  const outputIndex = args.indexOf("--output-dir");
  if (reportIndex < 0 || relationshipsIndex < 0) {
    throw new Error("usage: bun scripts/quality-metrics.mjs --report PATH --relationships PATH [--output-dir PATH]");
  }
  const reportPath = path.resolve(repoRoot, args[reportIndex + 1]);
  const relationshipsPath = path.resolve(repoRoot, args[relationshipsIndex + 1]);
  const outputDir = outputIndex < 0 ? path.join(repoRoot, "benchmarks/results") : path.resolve(repoRoot, args[outputIndex + 1]);
  const report = JSON.parse(await readFile(reportPath, "utf8"));
  const relationships = JSON.parse(await readFile(relationshipsPath, "utf8"));
  const summary = evaluateQualityMetrics(report, relationships);
  await mkdir(outputDir, { recursive: true });
  await writeFile(path.join(outputDir, "asset-tooling-quality-report.json"), `${JSON.stringify(summary, null, 2)}\n`);
  await writeFile(path.join(outputDir, "asset-tooling-quality-report.md"), qualityMetricsMarkdown(summary));
  process.stdout.write(`${qualityMetricsMarkdown(summary)}\n`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  cli().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}

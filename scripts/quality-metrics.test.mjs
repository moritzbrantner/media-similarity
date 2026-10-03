import { describe, expect, test } from "bun:test";

import { evaluateQualityMetrics, validateQualityRelationships } from "./quality-metrics.mjs";

const relationships = {
  version: 1,
  metrics: { recallK: [1, 5], ndcgK: 5, hardNegativeK: 5 },
  searches: [
    {
      searchId: "query-a",
      family: "face-identity",
      queryAssetId: "query-a-asset",
      relevance: [
        { assetId: "source-a", grade: 3 },
        { assetId: "source-a-variant", grade: 2 },
      ],
      hardNegatives: ["source-b"],
    },
    {
      searchId: "query-b",
      family: "crop-resize",
      queryAssetId: "query-b-asset",
      relevance: [{ assetId: "source-b", grade: 3 }],
      hardNegatives: ["source-a"],
    },
  ],
};

describe("quality metrics", () => {
  test("computes recall, reciprocal rank, nDCG and hard-negative rate from evaluator rankings", () => {
    const result = evaluateQualityMetrics(
      {
        corpus: "fixture",
        searches: [
          {
            id: "query-a",
            passed: true,
            top_k: [
              { asset_id: "source-a" },
              { asset_id: "source-b" },
              { asset_id: "source-a-variant" },
            ],
          },
          {
            id: "query-b",
            passed: false,
            top_k: [{ asset_id: "source-a" }, { asset_id: "source-b" }],
          },
        ],
      },
      relationships,
    );

    const first = result.cases[0];
    expect(first.recall.at1).toBe(0.5);
    expect(first.recall.at5).toBe(1);
    expect(first.reciprocalRank).toBe(1);
    expect(first.hardNegativeFalsePositiveRate).toBe(1);
    expect(first.ndcg).toBeGreaterThan(0.9);

    const second = result.cases[1];
    expect(second.recall.at1).toBe(0);
    expect(second.recall.at5).toBe(1);
    expect(second.reciprocalRank).toBe(0.5);
    expect(result.aggregate.mrr).toBe(0.75);
    expect(result.aggregate.evaluatorPassRate).toBe(0.5);
  });

  test("rejects relationships that label the same asset relevant and hard-negative", () => {
    const invalid = structuredClone(relationships);
    invalid.searches[0].hardNegatives = ["source-a"];
    expect(() => validateQualityRelationships(invalid)).toThrow(/relevant and hard-negative/);
  });
});

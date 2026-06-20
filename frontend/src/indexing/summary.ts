import type { IndexResponse } from "../types";

type IndexSummaryCounts = Pick<
  IndexResponse,
  "already_indexed" | "failed" | "indexed" | "pruned" | "skipped"
>;

export function formatIndexSummary(index: IndexSummaryCounts) {
  return `Indexed ${index.indexed} media item(s), already indexed ${index.already_indexed}, skipped ${index.skipped}, pruned ${index.pruned}, failed ${index.failed}.`;
}

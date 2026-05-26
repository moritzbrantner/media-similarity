import type { SearchResult } from "../types";
import { captureTimeMs } from "./filtering";
import type { ResultSortMode } from "./types";

export function sortResults(results: SearchResult[], sortMode: ResultSortMode) {
  return results
    .map((result, index) => ({ index, result }))
    .sort((left, right) => {
      const comparison = compareResults(left.result, right.result, sortMode);
      return comparison === 0 ? left.index - right.index : comparison;
    })
    .map(({ result }) => result);
}

function compareResults(left: SearchResult, right: SearchResult, sortMode: ResultSortMode) {
  switch (sortMode) {
    case "captured_newest":
      return compareCapturedNewest(left, right);
    case "filename":
      return compareFilenames(left, right);
    case "modified_newest":
      return compareDescending(left.image.modified_at, right.image.modified_at, left, right);
    case "size_largest":
      return compareDescending(left.image.size_bytes, right.image.size_bytes, left, right);
    case "vector_score":
      return compareDescending(left.vector_score, right.vector_score, left, right);
    case "phash_distance":
      return compareHashDistance(left, right);
  }
}

function compareCapturedNewest(left: SearchResult, right: SearchResult) {
  const leftCaptured = captureTimeMs(left.image.photo_metadata?.capture_time ?? null);
  const rightCaptured = captureTimeMs(right.image.photo_metadata?.capture_time ?? null);

  if (leftCaptured === null && rightCaptured === null) {
    return compareHashDistanceForTie(left, right);
  }

  if (leftCaptured === null) {
    return 1;
  }

  if (rightCaptured === null) {
    return -1;
  }

  return rightCaptured - leftCaptured || compareHashDistanceForTie(left, right);
}

function compareHashDistance(left: SearchResult, right: SearchResult) {
  const leftDistance = left.hash_distance;
  const rightDistance = right.hash_distance;

  if (leftDistance === null && rightDistance === null) {
    return compareDescending(left.vector_score, right.vector_score, left, right);
  }

  if (leftDistance === null) {
    return 1;
  }

  if (rightDistance === null) {
    return -1;
  }

  return (
    leftDistance - rightDistance ||
    compareDescending(left.vector_score, right.vector_score, left, right)
  );
}

function compareDescending(
  leftValue: number,
  rightValue: number,
  leftResult: SearchResult,
  rightResult: SearchResult,
) {
  return rightValue - leftValue || compareHashDistanceForTie(leftResult, rightResult);
}

function compareHashDistanceForTie(left: SearchResult, right: SearchResult) {
  const leftDistance = left.hash_distance;
  const rightDistance = right.hash_distance;

  if (leftDistance === null && rightDistance === null) {
    return compareFilenames(left, right);
  }

  if (leftDistance === null) {
    return 1;
  }

  if (rightDistance === null) {
    return -1;
  }

  return leftDistance - rightDistance || compareFilenames(left, right);
}

function compareFilenames(left: SearchResult, right: SearchResult) {
  return (
    left.image.filename.localeCompare(right.image.filename, undefined, {
      sensitivity: "base",
    }) ||
    left.image.relative_path.localeCompare(right.image.relative_path, undefined, {
      sensitivity: "base",
    })
  );
}

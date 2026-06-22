import type { MetadataFilters, ResultSortMode } from "./types";

export const DEFAULT_LIMIT = 12;
export const DEFAULT_RESULT_SORT: ResultSortMode = "relevance";
export const MAX_SEARCH_HISTORY = 8;
export const SEARCH_HISTORY_STORAGE_KEY = "image-similarity-search-history";
export const SEARCH_HISTORY_QUERY_KEY = ["search-history"] as const;
export const DEFAULT_METADATA_FILTERS = {
  cameraQuery: "",
  captureDateFrom: "",
  captureDateTo: "",
  dateFrom: "",
  dateTo: "",
  hasGps: "all",
  keywordQuery: "",
  maxHeight: "",
  maxSizeMb: "",
  maxWidth: "",
  mediaKind: "all",
  minHeight: "",
  minSizeMb: "",
  minWidth: "",
  nameQuery: "",
  nearDuplicate: "all",
  orientation: "all",
  personId: "",
  sourceType: "all",
} satisfies MetadataFilters;

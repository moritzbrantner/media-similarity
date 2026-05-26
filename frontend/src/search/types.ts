import type { SearchResponse } from "../types";

export type MetadataFilters = {
  cameraQuery: string;
  captureDateFrom: string;
  captureDateTo: string;
  dateFrom: string;
  dateTo: string;
  hasGps: "all" | "yes" | "no";
  keywordQuery: string;
  maxHeight: string;
  maxSizeMb: string;
  maxWidth: string;
  mediaKind:
    | "all"
    | "static_image"
    | "animated_gif"
    | "video_scene"
    | "audio"
    | "pdf_page"
    | "pdf_document";
  minHeight: string;
  minSizeMb: string;
  minWidth: string;
  nameQuery: string;
  nearDuplicate: "all" | "exclude" | "only";
  orientation: "all" | "landscape" | "portrait" | "square";
  personId: string;
  sourceType: string;
};

export type ResultSortMode =
  | "captured_newest"
  | "filename"
  | "modified_newest"
  | "phash_distance"
  | "size_largest"
  | "vector_score";

export type SearchHistoryItem = {
  id: string;
  fileName: string;
  filters: MetadataFilters;
  limit: number;
  ocrTextQuery: string;
  queryImageUrl: string | null;
  queryMediaKind: SearchResponse["query_media_kind"];
  sortMode: ResultSortMode;
  searchedAt: string;
  response: SearchResponse;
};

export type SearchVariables = {
  filters: MetadataFilters;
  ocrTextQuery: string;
  queryFile: File;
  queryImageUrl: string | null;
  resultLimit: number;
  sortMode: ResultSortMode;
};

export type AppView = "configure" | "indexing" | "inverse-index" | "search";

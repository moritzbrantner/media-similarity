import type { SearchResponse } from "../types";
import { parseResponse } from "./client";

export type SearchMediaFilters = {
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
  mediaKind: string;
  minHeight: string;
  minSizeMb: string;
  minWidth: string;
  nameQuery: string;
  nearDuplicate: "all" | "exclude" | "only";
  orientation: "all" | "landscape" | "portrait" | "square";
  personId: string;
  sourceType: string;
};

export async function searchMedia(
  file: File,
  limit: number,
  ocrText: string,
  filters: SearchMediaFilters,
): Promise<SearchResponse> {
  const formData = new FormData();
  formData.append("file", file);
  const params = new URLSearchParams({ limit: String(limit) });
  const normalizedOcrText = ocrText.trim();
  if (normalizedOcrText) {
    params.set("ocr_text", normalizedOcrText);
  }
  appendSearchFilterParams(params, filters);

  const response = await fetch(`/api/search?${params.toString()}`, {
    body: formData,
    method: "POST",
  });
  return parseResponse<SearchResponse>(response);
}

function appendSearchFilterParams(params: URLSearchParams, filters: SearchMediaFilters) {
  appendStringParam(params, "source_type", filters.sourceType, "all");
  appendStringParam(params, "media_kind", filters.mediaKind, "all");
  appendStringParam(params, "name_query", filters.nameQuery);
  appendStringParam(params, "camera_query", filters.cameraQuery);
  appendStringParam(params, "keyword_query", filters.keywordQuery);
  appendStringParam(params, "has_gps", filters.hasGps, "all");
  appendStringParam(params, "near_duplicate", filters.nearDuplicate, "all");
  appendStringParam(params, "orientation", filters.orientation, "all");
  appendStringParam(params, "person_id", filters.personId);
  appendNumberParam(params, "min_width", filters.minWidth);
  appendNumberParam(params, "max_width", filters.maxWidth);
  appendNumberParam(params, "min_height", filters.minHeight);
  appendNumberParam(params, "max_height", filters.maxHeight);
  appendSizeParam(params, "min_size_bytes", filters.minSizeMb);
  appendSizeParam(params, "max_size_bytes", filters.maxSizeMb);
  appendDateParam(params, "modified_from", filters.dateFrom, "start");
  appendDateParam(params, "modified_to", filters.dateTo, "end");
  appendDateParam(params, "captured_from", filters.captureDateFrom, "start");
  appendDateParam(params, "captured_to", filters.captureDateTo, "end");
}

function appendStringParam(
  params: URLSearchParams,
  name: string,
  value: string,
  ignoredValue = "",
) {
  const normalized = value.trim();
  if (normalized && normalized !== ignoredValue) {
    params.set(name, normalized);
  }
}

function appendNumberParam(params: URLSearchParams, name: string, value: string) {
  const normalized = value.trim();
  if (!normalized) {
    return;
  }
  const parsed = Number(normalized);
  if (Number.isFinite(parsed) && parsed >= 0) {
    params.set(name, String(parsed));
  }
}

function appendSizeParam(params: URLSearchParams, name: string, value: string) {
  const normalized = value.trim();
  if (!normalized) {
    return;
  }
  const parsed = Number(normalized);
  if (Number.isFinite(parsed) && parsed >= 0) {
    params.set(name, String(Math.round(parsed * 1024 * 1024)));
  }
}

function appendDateParam(
  params: URLSearchParams,
  name: string,
  value: string,
  boundary: "end" | "start",
) {
  if (!value) {
    return;
  }
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return;
  }
  if (boundary === "end") {
    date.setDate(date.getDate() + 1);
    date.setMilliseconds(date.getMilliseconds() - 1);
  }
  params.set(name, String(date.getTime() / 1000));
}

import type {
  HealthResponse,
  IndexResponse,
  InverseIndexResponse,
  JobEvent,
  JobSnapshot,
  DeleteIndexResponse,
  EditableSmartAlbum,
  ModelsResponse,
  ImagePayload,
  SearchResponse,
  SmartAlbum,
  SmartAlbumResultsResponse,
  SourceConfigResponse,
  SourceIndexingConfig,
  ValidateWorkflowResponse,
  WorkflowConfigResponse,
  WorkflowEditorLibrary,
} from "./types";
import type { MediaWorkflowNodeData } from "./types";

async function parseResponse<T>(response: Response): Promise<T> {
  const text = await response.text();
  const payload = text ? tryParseJson(text) : null;

  if (!response.ok) {
    const parsedDetail = errorDetail(payload);
    const detail = parsedDetail ?? (text ? text : `${response.status} ${response.statusText}`);
    throw new Error(detail);
  }

  return payload as T;
}

function tryParseJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function errorDetail(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }

  if (!("detail" in payload)) {
    return null;
  }

  const detail = payload.detail;
  if (typeof detail === "string") {
    return detail;
  }

  if (Array.isArray(detail)) {
    return detail
      .map((item) => {
        if (item && typeof item === "object" && "msg" in item && typeof item.msg === "string") {
          return item.msg;
        }
        return JSON.stringify(item);
      })
      .join("; ");
  }

  return null;
}

export async function fetchHealth(): Promise<HealthResponse> {
  const response = await fetch("/api/health");
  return parseResponse<HealthResponse>(response);
}

export async function fetchInverseIndex(): Promise<InverseIndexResponse> {
  const response = await fetch("/api/inverse-index");
  return parseResponse<InverseIndexResponse>(response);
}

export async function fetchSmartAlbums(): Promise<{ albums: SmartAlbum[] }> {
  const response = await fetch("/api/smart-albums");
  return parseResponse<{ albums: SmartAlbum[] }>(response);
}

export async function createSmartAlbum(input: EditableSmartAlbum): Promise<SmartAlbum> {
  const response = await fetch("/api/smart-albums", {
    body: JSON.stringify(input),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  });
  return parseResponse<SmartAlbum>(response);
}

export async function updateSmartAlbum(id: string, input: EditableSmartAlbum): Promise<SmartAlbum> {
  const response = await fetch(`/api/smart-albums/${encodeURIComponent(id)}`, {
    body: JSON.stringify(input),
    headers: { "Content-Type": "application/json" },
    method: "PUT",
  });
  return parseResponse<SmartAlbum>(response);
}

export async function deleteSmartAlbum(id: string): Promise<{ deleted: boolean }> {
  const response = await fetch(`/api/smart-albums/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
  return parseResponse<{ deleted: boolean }>(response);
}

export async function fetchSmartAlbumResults(
  id: string,
  offset = 0,
  limit?: number,
): Promise<SmartAlbumResultsResponse> {
  const params = new URLSearchParams({ offset: String(offset) });
  if (limit) {
    params.set("limit", String(limit));
  }
  const response = await fetch(`/api/smart-albums/${encodeURIComponent(id)}/results?${params}`);
  return parseResponse<SmartAlbumResultsResponse>(response);
}

export async function previewSmartAlbum(
  input: EditableSmartAlbum,
  offset = 0,
  limit?: number,
): Promise<SmartAlbumResultsResponse> {
  const params = new URLSearchParams({ offset: String(offset) });
  if (limit) {
    params.set("limit", String(limit));
  }
  const response = await fetch(`/api/smart-albums/preview?${params}`, {
    body: JSON.stringify(input),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  });
  return parseResponse<SmartAlbumResultsResponse>(response);
}

export type IdentityKind = "person" | "speaker";

export type IdentityMutationResponse = {
  kind: IdentityKind;
  target_id: string;
  target_label: string | null;
  source_ids: string[];
  updated_media: number;
  updated_faces: number;
  registry_updated: boolean;
  warnings: string[];
};

export async function renameIdentity(
  kind: IdentityKind,
  id: string,
  label: string,
): Promise<IdentityMutationResponse> {
  const response = await fetch(`${identityRoute(kind)}/${encodeURIComponent(id)}`, {
    body: JSON.stringify({ label }),
    headers: { "Content-Type": "application/json" },
    method: "PUT",
  });
  return parseResponse<IdentityMutationResponse>(response);
}

export async function mergeIdentities(
  kind: IdentityKind,
  targetId: string,
  sourceIds: string[],
): Promise<IdentityMutationResponse> {
  const response = await fetch(`${identityRoute(kind)}/${encodeURIComponent(targetId)}/merge`, {
    body: JSON.stringify({ source_ids: sourceIds }),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  });
  return parseResponse<IdentityMutationResponse>(response);
}

function identityRoute(kind: IdentityKind) {
  return kind === "person" ? "/api/identities/people" : "/api/identities/speakers";
}

export async function indexSources(): Promise<IndexResponse> {
  const response = await fetch("/api/index", { method: "POST" });
  return parseResponse<IndexResponse>(response);
}

export async function startIndexJob(): Promise<JobSnapshot> {
  const response = await fetch("/api/jobs/index", { method: "POST" });
  return parseResponse<JobSnapshot>(response);
}

export async function fetchJobs(): Promise<JobSnapshot[]> {
  const response = await fetch("/api/jobs");
  return parseResponse<JobSnapshot[]>(response);
}

export async function fetchModels(): Promise<ModelsResponse> {
  const response = await fetch("/api/models");
  return parseResponse<ModelsResponse>(response);
}

export async function downloadModel(role: string, model?: string | null): Promise<JobSnapshot> {
  const response = await fetch(`/api/models/${encodeURIComponent(role)}/download`, {
    body: JSON.stringify({ model: model ?? null }),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  });
  return parseResponse<JobSnapshot>(response);
}

export async function downloadAllModels(): Promise<JobSnapshot> {
  const response = await fetch("/api/models/download-all", { method: "POST" });
  return parseResponse<JobSnapshot>(response);
}

export async function enableModel(role: string, model?: string | null): Promise<JobSnapshot> {
  const response = await fetch(`/api/models/${encodeURIComponent(role)}/enable`, {
    body: JSON.stringify({ model: model ?? null }),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  });
  return parseResponse<JobSnapshot>(response);
}

export async function deleteIndexedMedia(id: string): Promise<DeleteIndexResponse> {
  const response = await fetch(`/api/indexed-media/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
  return parseResponse<DeleteIndexResponse>(response);
}

export async function updateIndexedMediaTags({
  id,
  tags,
}: {
  id: string;
  tags: string[];
}): Promise<ImagePayload> {
  const response = await fetch(`/api/indexed-media/${encodeURIComponent(id)}/tags`, {
    body: JSON.stringify({ tags }),
    headers: { "Content-Type": "application/json" },
    method: "PUT",
  });
  return parseResponse<ImagePayload>(response);
}

export async function deleteIndexedSource(filter: {
  source_item_uri?: string;
  source_uri?: string;
}): Promise<DeleteIndexResponse> {
  const params = new URLSearchParams();
  if (filter.source_uri) {
    params.set("source_uri", filter.source_uri);
  }
  if (filter.source_item_uri) {
    params.set("source_item_uri", filter.source_item_uri);
  }
  const response = await fetch(`/api/indexed-sources?${params.toString()}`, {
    method: "DELETE",
  });
  return parseResponse<DeleteIndexResponse>(response);
}

export async function fetchJobEvents(jobId: string): Promise<JobEvent[]> {
  const response = await fetch(`/api/jobs/${encodeURIComponent(jobId)}/events`);
  return parseResponse<JobEvent[]>(response);
}

export async function cancelJob(jobId: string): Promise<JobSnapshot> {
  const response = await fetch(`/api/jobs/${encodeURIComponent(jobId)}/cancel`, { method: "POST" });
  return parseResponse<JobSnapshot>(response);
}

export async function fetchSourceConfig(): Promise<SourceConfigResponse> {
  const response = await fetch("/api/source-config");
  return parseResponse<SourceConfigResponse>(response);
}

export async function updateSourceConfig(sources: string[]): Promise<SourceConfigResponse> {
  const response = await fetch("/api/source-config", {
    body: JSON.stringify({ sources }),
    headers: { "Content-Type": "application/json" },
    method: "PUT",
  });
  return parseResponse<SourceConfigResponse>(response);
}

export async function updateIndexingConfig(
  indexing: SourceIndexingConfig,
): Promise<SourceConfigResponse> {
  const response = await fetch("/api/source-config", {
    body: JSON.stringify({ indexing }),
    headers: { "Content-Type": "application/json" },
    method: "PUT",
  });
  return parseResponse<SourceConfigResponse>(response);
}

export async function fetchWorkflows(): Promise<WorkflowConfigResponse> {
  const response = await fetch("/api/workflows");
  return parseResponse<WorkflowConfigResponse>(response);
}

export async function updateWorkflows(
  library: WorkflowEditorLibrary<MediaWorkflowNodeData>,
): Promise<WorkflowConfigResponse> {
  const response = await fetch("/api/workflows", {
    body: JSON.stringify({ library }),
    headers: { "Content-Type": "application/json" },
    method: "PUT",
  });
  return parseResponse<WorkflowConfigResponse>(response);
}

export async function validateWorkflows(
  library: WorkflowEditorLibrary<MediaWorkflowNodeData>,
): Promise<ValidateWorkflowResponse> {
  const response = await fetch("/api/workflows/validate", {
    body: JSON.stringify({ library }),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  });
  return parseResponse<ValidateWorkflowResponse>(response);
}

export async function resetWorkflows(): Promise<WorkflowConfigResponse> {
  const response = await fetch("/api/workflows/reset", { method: "POST" });
  return parseResponse<WorkflowConfigResponse>(response);
}

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
  const parsed = Number(value);
  if (Number.isFinite(parsed) && parsed >= 0) {
    params.set(name, String(parsed));
  }
}

function appendSizeParam(params: URLSearchParams, name: string, value: string) {
  const parsed = Number(value);
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

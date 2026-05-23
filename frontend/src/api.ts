import type {
  HealthResponse,
  IndexResponse,
  InverseIndexResponse,
  JobEvent,
  JobSnapshot,
  DeleteIndexResponse,
  ModelsResponse,
  ImagePayload,
  SearchResponse,
  SourceConfigResponse,
  SourceIndexingConfig,
} from "./types";

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

export async function searchMedia(
  file: File,
  limit: number,
  ocrText: string,
  personId: string,
): Promise<SearchResponse> {
  const formData = new FormData();
  formData.append("file", file);
  const params = new URLSearchParams({ limit: String(limit) });
  const normalizedOcrText = ocrText.trim();
  if (normalizedOcrText) {
    params.set("ocr_text", normalizedOcrText);
  }
  const normalizedPersonId = personId.trim();
  if (normalizedPersonId) {
    params.set("person_id", normalizedPersonId);
  }

  const response = await fetch(`/api/search?${params.toString()}`, {
    body: formData,
    method: "POST",
  });
  return parseResponse<SearchResponse>(response);
}

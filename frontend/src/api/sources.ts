import type {
  DeleteIndexResponse,
  HealthResponse,
  ImagePayload,
  SourceConfigResponse,
  SourceIndexingConfig,
} from "../types";
import type { InverseIndexResponse } from "../types";
import { parseResponse } from "./client";

export async function fetchHealth(): Promise<HealthResponse> {
  const response = await fetch("/api/health");
  return parseResponse<HealthResponse>(response);
}

export async function fetchInverseIndex(): Promise<InverseIndexResponse> {
  const response = await fetch("/api/inverse-index");
  return parseResponse<InverseIndexResponse>(response);
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

export async function indexSources(): Promise<{ indexed: number }> {
  const response = await fetch("/api/index", { method: "POST" });
  return parseResponse<{ indexed: number }>(response);
}

import type { HealthResponse, IndexResponse, SearchResponse } from "./types";

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

export async function indexSources(): Promise<IndexResponse> {
  const response = await fetch("/api/index", { method: "POST" });
  return parseResponse<IndexResponse>(response);
}

export async function searchMedia(
  file: File,
  limit: number,
  ocrText: string,
): Promise<SearchResponse> {
  const formData = new FormData();
  formData.append("file", file);
  const params = new URLSearchParams({ limit: String(limit) });
  const normalizedOcrText = ocrText.trim();
  if (normalizedOcrText) {
    params.set("ocr_text", normalizedOcrText);
  }

  const response = await fetch(`/api/search?${params.toString()}`, {
    body: formData,
    method: "POST",
  });
  return parseResponse<SearchResponse>(response);
}

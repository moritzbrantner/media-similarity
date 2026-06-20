import type {
  EditableSmartAlbum,
  SmartAlbum,
  SmartAlbumResultsResponse,
} from "../types";
import { parseResponse } from "./client";

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

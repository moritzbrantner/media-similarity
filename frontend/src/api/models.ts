import type { JobSnapshot, ModelsResponse } from "../types";
import { parseResponse } from "./client";

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

export async function disableModel(role: string): Promise<JobSnapshot> {
  const response = await fetch(`/api/models/${encodeURIComponent(role)}/disable`, {
    method: "POST",
  });
  return parseResponse<JobSnapshot>(response);
}

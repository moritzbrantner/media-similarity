import type { DeleteIndexResponse, JobEvent, JobSnapshot } from "../types";
import { parseResponse } from "./client";

export async function startIndexJob(): Promise<JobSnapshot> {
  const response = await fetch("/api/jobs/index", { method: "POST" });
  return parseResponse<JobSnapshot>(response);
}

export async function fetchJobs(): Promise<JobSnapshot[]> {
  const response = await fetch("/api/jobs");
  return parseResponse<JobSnapshot[]>(response);
}

export async function fetchJobEvents(jobId: string): Promise<JobEvent[]> {
  const response = await fetch(`/api/jobs/${encodeURIComponent(jobId)}/events`);
  return parseResponse<JobEvent[]>(response);
}

export async function cancelJob(jobId: string): Promise<JobSnapshot> {
  const response = await fetch(`/api/jobs/${encodeURIComponent(jobId)}/cancel`, { method: "POST" });
  return parseResponse<JobSnapshot>(response);
}

export async function deleteIndexedMedia(id: string): Promise<DeleteIndexResponse> {
  const response = await fetch(`/api/indexed-media/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
  return parseResponse<DeleteIndexResponse>(response);
}

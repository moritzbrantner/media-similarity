import type {
  ValidateWorkflowResponse,
  WorkflowConfigResponse,
  WorkflowEditorLibrary,
  MediaWorkflowNodeData,
} from "../types";
import { parseResponse } from "./client";

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

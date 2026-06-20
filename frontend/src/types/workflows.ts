import type {
  WorkflowEditorDocument,
  WorkflowEditorNodeTemplate,
} from "@moritzbrantner/workflow-editor";

export type MediaWorkflowProcessor =
  | "source.input"
  | "image.decode"
  | "gif.decode"
  | "video.detect_scenes"
  | "video.split_scenes"
  | "audio.decode_segments"
  | "pdf.render_pages"
  | "pdf.build_document_summary"
  | "photo.extract_metadata"
  | "ocr.extract"
  | "faces.analyze"
  | "audio.analyze"
  | "thumbnail.ensure"
  | "thumbnail.ensure_animated"
  | "embedding.visual"
  | "payload.build"
  | "qdrant.upsert";

export type MediaWorkflowNodeData = {
  processor: MediaWorkflowProcessor | string;
  enabled?: boolean;
  config?: Record<string, unknown>;
  locked?: boolean;
};

export type WorkflowEditorLibraryEntry<TNodeData = Record<string, unknown>> = {
  id: string;
  name: string;
  description?: string | null;
  version: number;
  createdAt: string;
  updatedAt: string;
  document: WorkflowEditorDocument<TNodeData>;
  versions: unknown[];
};

export type WorkflowEditorLibrary<TNodeData = Record<string, unknown>> = {
  format: string;
  version: number;
  activeDocumentId?: string | null;
  documents: WorkflowEditorLibraryEntry<TNodeData>[];
};

export type MediaWorkflowTypeDefinition = {
  name: string;
  type: unknown;
};

export type WorkflowDiagnostic = {
  code: string;
  message: string;
  document_id?: string | null;
  node_id?: string | null;
  edge_id?: string | null;
};

export type WorkflowConfigResponse = {
  workflow_file: string;
  writable: boolean;
  library: WorkflowEditorLibrary<MediaWorkflowNodeData>;
  node_templates: WorkflowEditorNodeTemplate<MediaWorkflowNodeData>[];
  type_definitions: MediaWorkflowTypeDefinition[];
  diagnostics: WorkflowDiagnostic[];
};

export type ValidateWorkflowResponse = {
  diagnostics: WorkflowDiagnostic[];
};

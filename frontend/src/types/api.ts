export type HealthResponse = {
  status: string;
  collection: string;
  source_dir: string;
  sources: string[];
};

export type IndexResponse = {
  indexed: number;
  already_indexed: number;
  skipped: number;
  failed: number;
  pruned: number;
  collection: string;
  source_dir: string;
  sources: string[];
  errors: string[];
};

export type DeleteIndexResponse = {
  deleted_points: number;
  deleted_faces: number;
  deleted_artifacts: number;
  errors: string[];
};

export type SourceConfigResponse = {
  media_sources_file: string;
  media_sources_seed_file: string | null;
  media_sources_writable: boolean;
  default_source_dir: string;
  sources: SourceConfigSource[];
  supported_source_types: SupportedSourceType[];
  indexing: SourceIndexingConfig;
};

export type SourceConfigSource = {
  spec: string;
  kind: string;
  status: "not_implemented" | "ready" | "unavailable" | "unsupported" | string;
  detail: string | null;
};

export type SupportedSourceType = {
  kind: string;
  label: string;
  implemented: boolean;
  example: string;
};

export type SourceIndexingConfig = {
  collection: string;
  image_extensions: string[];
  audio_extensions: string[];
  pdf_extensions: string[];
  video_extensions: string[];
  visual_embedding_enabled: boolean;
  visual_embedding_model: string;
  visual_embedding_vector_size: number;
  face_analysis_enabled: boolean;
  face_detection_min_confidence: number;
  face_cluster_threshold: number;
  face_min_cluster_images: number;
  face_max_frames_per_media: number;
  gif_sample_frames: number;
  gif_max_decode_frames: number;
  gif_preview_frames: number;
  gif_default_frame_delay_ms: number;
  gif_motion_weight: number;
  video_frame_stride: number;
  video_max_frames: number | null;
  pdf_render_dpi: number;
  pdf_max_pages: number;
  pdf_summary_pages: number;
  ocr_enabled: boolean;
  ocr_max_frames: number;
  audio_transcription_enabled: boolean;
};

export type ModelOption = {
  id: string;
  label: string;
  cached: boolean;
  configured: boolean;
};

export type ModelRuntimeStatus = {
  role: string;
  label: string;
  configured: string;
  cached: boolean;
  active: boolean;
  blocking: boolean;
  required_action: "download" | "enable" | null;
  bundle_path: string | null;
  detail: string | null;
  options: ModelOption[];
};

export type ModelsResponse = {
  models: ModelRuntimeStatus[];
};

export type AudioTranscriptionModelResponse = {
  id: string;
  cached: boolean;
  configured: boolean;
};

export type AudioTranscriptionModelsResponse = {
  enabled: boolean;
  provider: string;
  configured_model: string;
  device: string;
  compute_type: string;
  language: string | null;
  batch_chunks: boolean;
  max_batch_size: number | null;
  auto_download: boolean;
  cache_dir: string | null;
  models: AudioTranscriptionModelResponse[];
};

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

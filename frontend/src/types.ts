import type {
  WorkflowEditorDocument,
  WorkflowEditorNodeTemplate,
} from "@moritzbrantner/workflow-editor";

export type HealthResponse = {
  status: string;
  collection: string;
  source_dir: string;
  sources: string[];
};

export type IndexResponse = {
  indexed: number;
  skipped: number;
  failed: number;
  pruned: number;
  collection: string;
  source_dir: string;
  sources: string[];
  errors: string[];
};

export type JobStatus = "Queued" | "Running" | "Cancelling" | "Succeeded" | "Failed" | "Cancelled";

export type JobProgress = {
  completed: number;
  total: number | null;
  unit: string;
  message: string | null;
};

export type JobLogEntry = {
  timestamp: string;
  level: "Debug" | "Info" | "Warn" | "Error";
  message: string;
};

export type JobSpec = {
  id: string;
  name: string;
  kind: string | null;
  metadata: Record<string, string>;
};

export type JobSnapshot = {
  spec: JobSpec;
  status: JobStatus;
  progress: JobProgress | null;
  logs: JobLogEntry[];
  artifacts: unknown[];
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  failure: { message: string } | null;
  metadata: Record<string, string>;
};

export type JobEvent = {
  job_id: string;
  sequence: number;
  timestamp: string;
  kind:
    | { StatusChanged: { status: JobStatus; message: string | null } }
    | { Progress: JobProgress }
    | { Log: JobLogEntry }
    | { Artifact: unknown }
    | { Metadata: { key: string; value: string } };
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

export type DeleteIndexResponse = {
  deleted_points: number;
  deleted_faces: number;
  deleted_artifacts: number;
  errors: string[];
};

export type InverseIndexLocation = {
  media_id: string;
  filename: string;
  relative_path: string;
  path: string;
  media_kind: ImagePayload["media_kind"];
  source_type: string;
  source_uri: string | null;
  source_item_uri: string | null;
  thumbnail_url: string | null;
  media_url: string | null;
  scene_clip_url: string | null;
  occurrence_count: number;
  frame_indices: number[];
  start_seconds: number | null;
  end_seconds: number | null;
  page_number: number | null;
  confidence: number;
};

export type InversePersonEntry = {
  id: string;
  label: string | null;
  face_count: number;
  media_count: number;
  confidence: number;
  locations: InverseIndexLocation[];
};

export type InverseSpeakerEntry = {
  id: string;
  label: string | null;
  segment_count: number;
  total_seconds: number;
  media_count: number;
  confidence: number;
  locations: InverseIndexLocation[];
};

export type InverseIndexResponse = {
  indexed_media: number;
  people: InversePersonEntry[];
  speakers: InverseSpeakerEntry[];
  errors: string[];
};

export type AudioAnalysis = {
  speech_detected: boolean;
  speech_ratio: number;
  speech_segments: AudioSpeechSegment[];
  audio_segments: AudioSegmentGuess[];
  recognized_voices: AudioRecognizedVoice[];
  transcript_text: string;
  transcript_language: string | null;
  transcript_segments: AudioTranscriptSegment[];
  tempo_bpm: number | null;
  tempo_confidence: number;
  tempo_onset_count: number;
};

export type AudioSpeechSegment = {
  start_seconds: number;
  end_seconds: number;
  confidence: number;
};

export type AudioSegmentGuess = {
  segment_index: number;
  kind: string;
  start_seconds: number;
  end_seconds: number;
  confidence: number;
  speaker_id: string | null;
  speaker_label: string | null;
};

export type AudioRecognizedVoice = {
  id: string;
  label: string;
  segment_count: number;
  total_seconds: number;
  confidence: number;
};

export type AudioTranscriptSegment = {
  segment_index: number;
  start_seconds: number | null;
  end_seconds: number | null;
  text: string;
  confidence: number | null;
};

export type OcrFrameText = {
  frame_index: number;
  text: string;
};

export type FaceBoxPayload = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type FaceDetectionPayload = {
  face_id: string;
  media_id: string;
  frame_index: number;
  bbox: FaceBoxPayload;
  confidence: number;
  person_id: string | null;
  person_label: string | null;
};

export type PersonSummary = {
  person_id: string;
  label: string | null;
  face_count: number;
  media_count: number;
  confidence: number;
};

export type PhotoGpsPayload = {
  latitude: number;
  longitude: number;
  altitude_meters: number | null;
};

export type PhotoMetadataEntryPayload = {
  namespace: string;
  key: string;
  label: string;
  value: string;
};

export type PhotoMetadataPayload = {
  capture_time: string | null;
  camera_make: string | null;
  camera_model: string | null;
  lens_model: string | null;
  orientation: string | null;
  gps: PhotoGpsPayload | null;
  rating: number | null;
  keywords: string[];
  title: string | null;
  description: string | null;
  creator: string | null;
  copyright: string | null;
  raw: PhotoMetadataEntryPayload[];
};

export type ImagePayload = {
  id: string;
  path: string;
  relative_path: string;
  filename: string;
  width: number;
  height: number;
  size_bytes: number;
  modified_at: number;
  phash: string;
  thumbnail_url: string | null;
  animated_thumbnail_url: string | null;
  media_kind:
    | "static_image"
    | "animated_gif"
    | "video_scene"
    | "audio"
    | "pdf_page"
    | "pdf_document";
  frame_count: number | null;
  duration_ms: number | null;
  full_video_url: string | null;
  full_audio_url: string | null;
  full_pdf_url: string | null;
  pdf_page_url: string | null;
  pdf_document_id: string | null;
  pdf_page_index: number | null;
  pdf_page_number: number | null;
  pdf_page_count: number | null;
  audio_analysis: AudioAnalysis | null;
  ocr_text: string;
  ocr_frames: OcrFrameText[];
  visual_embedding_model: string | null;
  faces: FaceDetectionPayload[];
  people: PersonSummary[];
  artifacts: { kind: string; url: string }[];
  tags: string[];
  photo_metadata: PhotoMetadataPayload | null;
  scene_clip_url: string | null;
  scene_index: number | null;
  scene_start_frame: number | null;
  scene_end_frame: number | null;
  scene_start_seconds: number | null;
  scene_end_seconds: number | null;
  source_type: string;
  source_item_uri: string | null;
  indexing_profile: string | null;
  source_uri: string | null;
};

export type SearchResult = {
  image: ImagePayload;
  vector_score: number;
  hash_distance: number | null;
  ocr_score: number | null;
  near_duplicate: boolean;
  query_scene_index: number | null;
};

export type SearchSceneResponse = {
  scene_index: number;
  scene_kind: string;
  start_frame: number;
  end_frame: number;
  start_seconds: number;
  end_seconds: number;
  clip_url: string | null;
  page_index: number | null;
  page_number: number | null;
  page_label: string | null;
  speaker_id: string | null;
  speaker_label: string | null;
  query_phash: string;
  count: number;
  results: SearchResult[];
};

export type SearchResponse = {
  query_phash: string;
  count: number;
  results: SearchResult[];
  query_media_kind: "static_image" | "animated_gif" | "video" | "audio" | "pdf";
  scenes: SearchSceneResponse[];
  query_audio_analysis: AudioAnalysis | null;
  query_ocr_text: string;
};

export type AlbumSortMode =
  | "captured_newest"
  | "duplicate_group_size"
  | "filename"
  | "modified_newest"
  | "size_largest";

export type SmartAlbumCriteria = {
  source_type: string | null;
  media_kind: ImagePayload["media_kind"] | "all" | null;
  name_query: string | null;
  camera_query: string | null;
  keyword_query: string | null;
  text_query: string | null;
  person_id: string | null;
  speaker_id: string | null;
  has_gps: boolean | null;
  duplicate_status: "all" | "only" | "exclude";
  orientation: "all" | "landscape" | "portrait" | "square" | null;
  min_width: number | null;
  max_width: number | null;
  min_height: number | null;
  max_height: number | null;
  min_size_bytes: number | null;
  max_size_bytes: number | null;
  modified_from: number | null;
  modified_to: number | null;
  captured_from: number | null;
  captured_to: number | null;
};

export type EditableSmartAlbum = {
  name: string;
  description: string | null;
  criteria: SmartAlbumCriteria;
  sort: AlbumSortMode;
  limit: number;
};

export type SmartAlbum = EditableSmartAlbum & {
  id: string;
  created_at: string;
  updated_at: string;
};

export type SmartAlbumResult = {
  image: ImagePayload;
  duplicate_group_id: string | null;
  duplicate_group_size: number;
};

export type SmartAlbumResultsResponse = {
  album: SmartAlbum;
  count: number;
  total: number;
  offset: number;
  limit: number;
  warnings: string[];
  duplicate_groups: Array<{
    id: string;
    size: number;
    representative_media_id: string;
    media_ids: string[];
  }>;
  results: SmartAlbumResult[];
};

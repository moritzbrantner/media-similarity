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

export type OcrFrameText = {
  frame_index: number;
  text: string;
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

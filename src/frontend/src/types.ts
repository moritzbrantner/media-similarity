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
  collection: string;
  source_dir: string;
  sources: string[];
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
  media_kind: "static_image" | "animated_gif" | "video_scene" | "audio";
  frame_count: number | null;
  duration_ms: number | null;
  full_video_url: string | null;
  full_audio_url: string | null;
  audio_analysis: AudioAnalysis | null;
  ocr_text: string;
  ocr_frames: OcrFrameText[];
  scene_clip_url: string | null;
  scene_index: number | null;
  scene_start_frame: number | null;
  scene_end_frame: number | null;
  scene_start_seconds: number | null;
  scene_end_seconds: number | null;
  source_type: string;
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
  query_media_kind: "static_image" | "animated_gif" | "video" | "audio";
  scenes: SearchSceneResponse[];
  query_audio_analysis: AudioAnalysis | null;
  query_ocr_text: string;
};

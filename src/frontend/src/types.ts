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
  media_kind: "static_image" | "animated_gif" | "video_scene";
  frame_count: number | null;
  duration_ms: number | null;
  full_video_url: string | null;
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
  near_duplicate: boolean;
  query_scene_index: number | null;
};

export type SearchSceneResponse = {
  scene_index: number;
  start_frame: number;
  end_frame: number;
  start_seconds: number;
  end_seconds: number;
  clip_url: string | null;
  query_phash: string;
  count: number;
  results: SearchResult[];
};

export type SearchResponse = {
  query_phash: string;
  count: number;
  results: SearchResult[];
  query_media_kind: "static_image" | "animated_gif" | "video";
  scenes: SearchSceneResponse[];
};

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
  source_type: string;
  source_uri: string | null;
};

export type SearchResult = {
  image: ImagePayload;
  vector_score: number;
  hash_distance: number | null;
  near_duplicate: boolean;
};

export type SearchResponse = {
  query_phash: string;
  count: number;
  results: SearchResult[];
};

export const pngPixel = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=",
  "base64",
);

export const imageUpload = {
  buffer: pngPixel,
  mimeType: "image/png",
  name: "query.png",
};

export const gifUpload = {
  buffer: Buffer.from("GIF89a"),
  mimeType: "image/gif",
  name: "query.gif",
};

export const videoUpload = {
  buffer: Buffer.from("mock video"),
  mimeType: "video/mp4",
  name: "query.mp4",
};

export const audioUpload = {
  buffer: Buffer.from("mock audio"),
  mimeType: "audio/mpeg",
  name: "query.mp3",
};

export const pdfUpload = {
  buffer: Buffer.from("%PDF-1.4\n"),
  mimeType: "application/pdf",
  name: "query.pdf",
};

export const historyStorageKey = "image-similarity-search-history";

type DeepPartial<T> = {
  [Key in keyof T]?: T[Key] extends object ? DeepPartial<T[Key]> : T[Key];
};

export function makeHealthResponse(overrides: Partial<typeof defaultHealthResponse> = {}) {
  return {
    ...defaultHealthResponse,
    ...overrides,
  };
}

export function makeIndexResponse(overrides: Partial<typeof defaultIndexResponse> = {}) {
  return {
    ...defaultIndexResponse,
    ...overrides,
  };
}

export function makeJob(overrides: DeepPartial<typeof defaultCompletedIndexJob> = {}) {
  return {
    ...defaultCompletedIndexJob,
    ...overrides,
    failure: overrides.failure ?? defaultCompletedIndexJob.failure,
    logs: overrides.logs ?? defaultCompletedIndexJob.logs,
    metadata: {
      ...defaultCompletedIndexJob.metadata,
      ...overrides.metadata,
    },
    progress: overrides.progress ?? defaultCompletedIndexJob.progress,
    spec: {
      ...defaultCompletedIndexJob.spec,
      ...overrides.spec,
      metadata: {
        ...defaultCompletedIndexJob.spec.metadata,
        ...overrides.spec?.metadata,
      },
    },
  };
}

export function makeJobEvents(job = defaultCompletedIndexJob) {
  return [
    {
      job_id: job.spec.id,
      kind: { StatusChanged: { message: null, status: "Queued" } },
      sequence: 1,
      timestamp: "2026-05-22T10:00:00Z",
    },
    {
      job_id: job.spec.id,
      kind: { Progress: job.progress },
      sequence: 2,
      timestamp: "2026-05-22T10:00:02Z",
    },
    {
      job_id: job.spec.id,
      kind: { Log: job.logs[0] },
      sequence: 3,
      timestamp: "2026-05-22T10:00:03Z",
    },
  ];
}

export function makeSourceConfigResponse(
  overrides: DeepPartial<typeof defaultSourceConfigResponse> = {},
) {
  return {
    ...defaultSourceConfigResponse,
    ...overrides,
    indexing: {
      ...defaultSourceConfigResponse.indexing,
      ...overrides.indexing,
    },
    sources: overrides.sources ?? defaultSourceConfigResponse.sources,
    supported_source_types:
      overrides.supported_source_types ?? defaultSourceConfigResponse.supported_source_types,
  };
}

export function makeResult(overrides: DeepPartial<typeof defaultResult> = {}) {
  return {
    ...defaultResult,
    ...overrides,
    image: {
      ...defaultResult.image,
      ...overrides.image,
    },
  };
}

export function makeScene(overrides: DeepPartial<typeof defaultScene> = {}) {
  return {
    ...defaultScene,
    ...overrides,
    results: overrides.results ?? defaultScene.results,
  };
}

export function makeSearchResponse(overrides: DeepPartial<typeof defaultSearchResponse> = {}) {
  return {
    ...defaultSearchResponse,
    ...overrides,
    results: overrides.results ?? defaultSearchResponse.results,
    scenes: overrides.scenes ?? defaultSearchResponse.scenes,
  };
}

const defaultHealthResponse = {
  collection: "image_similarity_test",
  source_dir: "/images",
  sources: ["/images", "/archive"],
  status: "ok",
};

const defaultIndexResponse = {
  collection: "image_similarity_test",
  errors: [],
  failed: 0,
  indexed: 3,
  pruned: 1,
  skipped: 1,
  source_dir: "/images",
  sources: ["/images", "/archive"],
};

const defaultCompletedIndexJob = {
  artifacts: [],
  created_at: "2026-05-22T10:00:00Z",
  failure: null as { message: string } | null,
  finished_at: "2026-05-22T10:00:03Z",
  logs: [
    {
      level: "Info",
      message: "indexing complete: 3 media item(s), 1 skipped, 1 pruned, 0 failed",
      timestamp: "2026-05-22T10:00:03Z",
    },
  ],
  metadata: {
    collection: "image_similarity_test",
    failed: "0",
    indexed: "3",
    pruned: "1",
    skipped: "1",
  },
  progress: {
    completed: 2,
    message: "indexed 2/2 pending source files",
    total: 2,
    unit: "files",
  },
  spec: {
    id: "index.manual.mock",
    kind: "index.manual",
    metadata: {
      collection: "image_similarity_test",
    },
    name: "Index media sources",
  },
  started_at: "2026-05-22T10:00:01Z",
  status: "Succeeded",
};

const defaultSourceConfigResponse = {
  default_source_dir: "/images",
  indexing: {
    audio_extensions: [".mp3", ".wav"],
    audio_transcription_enabled: false,
    collection: "image_similarity_test",
    face_analysis_enabled: true,
    face_cluster_threshold: 0.38,
    face_detection_min_confidence: 0.75,
    face_max_frames_per_media: 8,
    face_min_cluster_images: 2,
    gif_default_frame_delay_ms: 100,
    gif_max_decode_frames: 512,
    gif_motion_weight: 0.2,
    gif_preview_frames: 16,
    gif_sample_frames: 16,
    image_extensions: [".jpg", ".png", ".gif"],
    ocr_enabled: true,
    ocr_max_frames: 4,
    pdf_extensions: [".pdf"],
    pdf_max_pages: 100,
    pdf_render_dpi: 144,
    pdf_summary_pages: 8,
    video_extensions: [".mp4", ".mov"],
    video_frame_stride: 30,
    video_max_frames: null as number | null,
    visual_embedding_enabled: true,
    visual_embedding_model: "sentence-transformers/clip-ViT-B-32",
    visual_embedding_vector_size: 512,
  },
  media_sources_file: "config/media-sources.txt",
  sources: [
    {
      detail: null as string | null,
      kind: "local",
      spec: "/images",
      status: "ready",
    },
    {
      detail: null as string | null,
      kind: "local",
      spec: "/archive",
      status: "ready",
    },
  ],
  supported_source_types: [
    {
      example: "/images or local:///images",
      implemented: true,
      kind: "local",
      label: "Local folder",
    },
    {
      example: "minio://bucket/prefix",
      implemented: false,
      kind: "minio",
      label: "MinIO bucket",
    },
    {
      example: "video:///clips/demo.mp4",
      implemented: false,
      kind: "video",
      label: "Video stream",
    },
    {
      example: "camera://front-door",
      implemented: false,
      kind: "camera",
      label: "Camera",
    },
  ],
};

const defaultImage = {
  animated_thumbnail_url: null as string | null,
  audio_analysis: null as unknown,
  duration_ms: null as number | null,
  faces: [],
  filename: "sunrise.jpg",
  frame_count: null as number | null,
  full_audio_url: null as string | null,
  full_pdf_url: null as string | null,
  full_video_url: null as string | null,
  height: 720,
  id: "local-sunrise",
  indexing_profile: null as string | null,
  media_kind: "static_image",
  modified_at: 1_700_000_000,
  ocr_frames: [],
  ocr_text: "",
  path: "/images/trips/sunrise.jpg",
  pdf_document_id: null as string | null,
  pdf_page_count: null as number | null,
  pdf_page_index: null as number | null,
  pdf_page_number: null as number | null,
  pdf_page_url: null as string | null,
  people: [],
  phash: "0123456789abcdef",
  relative_path: "trips/sunrise.jpg",
  scene_clip_url: null as string | null,
  scene_end_frame: null as number | null,
  scene_end_seconds: null as number | null,
  scene_index: null as number | null,
  scene_start_frame: null as number | null,
  scene_start_seconds: null as number | null,
  size_bytes: 1_048_576,
  source_item_uri: null as string | null,
  source_type: "local",
  source_uri: null as string | null,
  thumbnail_url: "/thumbnails/sunrise.jpg",
  visual_embedding_model: null as string | null,
  width: 1280,
};

const defaultResult = {
  hash_distance: 2 as number | null,
  image: defaultImage,
  near_duplicate: true,
  ocr_score: null as number | null,
  query_scene_index: null as number | null,
  vector_score: 0.7123,
};

const defaultScene = {
  clip_url: null as string | null,
  count: 0,
  end_frame: 0,
  end_seconds: 0,
  page_index: null as number | null,
  page_label: null as string | null,
  page_number: null as number | null,
  query_phash: "0123456789abcdef",
  results: [] as Array<ReturnType<typeof makeResult>>,
  scene_index: 0,
  scene_kind: "video_scene",
  speaker_id: null as string | null,
  speaker_label: null as string | null,
  start_frame: 0,
  start_seconds: 0,
};

const defaultSearchResponse = {
  count: 2,
  query_audio_analysis: null as unknown,
  query_media_kind: "static_image",
  query_ocr_text: "",
  query_phash: "0123456789abcdef",
  results: [
    makeResult({
      hash_distance: 16,
      image: {
        filename: "portrait.png",
        height: 1400,
        id: "import-portrait",
        modified_at: 1_690_000_000,
        path: "/archive/portraits/portrait.png",
        phash: "ffffffffffffffff",
        relative_path: "portraits/portrait.png",
        size_bytes: 2_097_152,
        source_type: "import",
        source_uri: "local:///archive",
        thumbnail_url: "/thumbnails/portrait.png",
        width: 900,
      },
      near_duplicate: false,
      vector_score: 0.9876,
    }),
    makeResult(),
  ],
  scenes: [] as Array<ReturnType<typeof makeScene>>,
};

export const healthResponse = makeHealthResponse();
export const indexResponse = makeIndexResponse();
export const completedIndexJob = makeJob();
export const completedIndexEvents = makeJobEvents(completedIndexJob);
export const sourceConfigResponse = makeSourceConfigResponse();
export const searchResponse = makeSearchResponse();

export const sortableSearchResponse = makeSearchResponse({
  count: 4,
  results: [
    ...searchResponse.results,
    makeResult({
      hash_distance: null,
      image: {
        filename: "clip.mp4",
        full_video_url: "/media/clip.mp4",
        height: 1080,
        id: "import-clip",
        media_kind: "video_scene",
        modified_at: 1_710_000_000,
        path: "/archive/video/clip.mp4",
        phash: "",
        relative_path: "video/clip.mp4",
        scene_clip_url: "/clips/clip-scene.mp4",
        scene_end_frame: 240,
        scene_end_seconds: 10,
        scene_index: 0,
        scene_start_frame: 120,
        scene_start_seconds: 5,
        size_bytes: 5_242_880,
        source_type: "import",
        source_uri: "local:///archive",
        thumbnail_url: "/thumbnails/clip.png",
        width: 1920,
      },
      near_duplicate: false,
      vector_score: 0.9999,
    }),
    makeResult({
      hash_distance: 2,
      image: {
        filename: "logo.png",
        height: 512,
        id: "local-logo",
        modified_at: 1_670_000_000,
        path: "/images/design/logo.png",
        phash: "0123456789abcdee",
        relative_path: "design/logo.png",
        size_bytes: 262_144,
        thumbnail_url: "/thumbnails/logo.png",
        width: 512,
      },
      near_duplicate: true,
      vector_score: 0.7123,
    }),
  ],
});

const pdfResult = makeResult({
  hash_distance: 4,
  image: {
    filename: "invoice.pdf page 001",
    frame_count: 1,
    full_pdf_url: "/uploads/source-pdfs/invoice.pdf",
    height: 1200,
    id: "pdf-page-1",
    media_kind: "pdf_page",
    modified_at: 1_720_000_000,
    ocr_text: "Invoice total due",
    path: "/documents/invoice.pdf#page=1",
    pdf_document_id: "pdf-document",
    pdf_page_count: 2,
    pdf_page_index: 0,
    pdf_page_number: 1,
    pdf_page_url: "/uploads/source-pdfs/invoice.pdf#page=1",
    phash: "1111111111111111",
    relative_path: "invoice.pdf#page-001",
    size_bytes: 131_072,
    source_uri: "/documents",
    thumbnail_url: "/thumbnails/pdf-page-1.png",
    width: 900,
  },
  near_duplicate: true,
  ocr_score: 1,
  query_scene_index: 0,
  vector_score: 0.91,
});

export const pdfSearchResponse = makeSearchResponse({
  count: 1,
  query_media_kind: "pdf",
  query_ocr_text: "invoice",
  query_phash: "1111111111111111",
  results: [pdfResult],
  scenes: [
    makeScene({
      count: 1,
      end_frame: 1,
      page_index: 0,
      page_label: "Page 1",
      page_number: 1,
      query_phash: "1111111111111111",
      results: [pdfResult],
      scene_kind: "pdf_page",
      start_frame: 1,
    }),
  ],
});

import { expect, test, type Page } from "@playwright/test";
import {
  captureSearchRequests,
  installDefaultApiMocks,
  mockEndpointFailure,
  mockSearchResponse as mockSearchResponseRoute,
} from "./support/api-mocks";
import {
  audioUpload,
  gifUpload,
  historyStorageKey,
  imageUpload,
  makeJob,
  makeJobEvents,
  makeResult,
  makeScene,
  makeSearchResponse,
  makeSourceConfigResponse,
  pdfUpload,
  pngPixel,
  searchResponse as fixtureSearchResponse,
  videoUpload,
} from "./support/media-fixtures";

const sourceConfigResponse = {
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
    video_max_frames: null,
    visual_embedding_enabled: true,
    visual_embedding_model: "sentence-transformers/clip-ViT-B-32",
    visual_embedding_vector_size: 512,
  },
  media_sources_file: "config/media-sources.txt",
  sources: [
    {
      detail: null,
      kind: "local",
      spec: "/images",
      status: "ready",
    },
    {
      detail: null,
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

const searchResponse = {
  count: 2,
  query_audio_analysis: null,
  query_media_kind: "static_image",
  query_phash: "0123456789abcdef",
  results: [
    {
      hash_distance: 16,
      image: {
        animated_thumbnail_url: null,
        audio_analysis: null,
        duration_ms: null,
        filename: "portrait.png",
        frame_count: null,
        full_audio_url: null,
        full_video_url: null,
        height: 1400,
        id: "import-portrait",
        media_kind: "static_image",
        modified_at: 1_690_000_000,
        path: "/archive/portraits/portrait.png",
        phash: "ffffffffffffffff",
        relative_path: "portraits/portrait.png",
        scene_clip_url: null,
        scene_end_frame: null,
        scene_end_seconds: null,
        scene_index: null,
        scene_start_frame: null,
        scene_start_seconds: null,
        size_bytes: 2_097_152,
        source_type: "import",
        source_uri: "local:///archive",
        thumbnail_url: "/thumbnails/portrait.png",
        width: 900,
      },
      near_duplicate: false,
      query_scene_index: null,
      vector_score: 0.9876,
    },
    {
      hash_distance: 2,
      image: {
        animated_thumbnail_url: null,
        audio_analysis: null,
        duration_ms: null,
        filename: "sunrise.jpg",
        frame_count: null,
        full_audio_url: null,
        full_video_url: null,
        height: 720,
        id: "local-sunrise",
        media_kind: "static_image",
        modified_at: 1_700_000_000,
        path: "/images/trips/sunrise.jpg",
        phash: "0123456789abcdef",
        relative_path: "trips/sunrise.jpg",
        scene_clip_url: null,
        scene_end_frame: null,
        scene_end_seconds: null,
        scene_index: null,
        scene_start_frame: null,
        scene_start_seconds: null,
        size_bytes: 1_048_576,
        source_type: "local",
        source_uri: null,
        thumbnail_url: "/thumbnails/sunrise.jpg",
        width: 1280,
      },
      near_duplicate: true,
      query_scene_index: null,
      vector_score: 0.7123,
    },
  ],
  scenes: [],
};

const sortableSearchResponse = {
  ...searchResponse,
  count: 4,
  results: [
    ...searchResponse.results,
    {
      hash_distance: null,
      image: {
        animated_thumbnail_url: null,
        audio_analysis: null,
        duration_ms: null,
        filename: "clip.mp4",
        frame_count: null,
        full_audio_url: null,
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
      query_scene_index: null,
      vector_score: 0.9999,
    },
    {
      hash_distance: 2,
      image: {
        animated_thumbnail_url: null,
        audio_analysis: null,
        duration_ms: null,
        filename: "logo.png",
        frame_count: null,
        full_audio_url: null,
        full_video_url: null,
        height: 512,
        id: "local-logo",
        media_kind: "static_image",
        modified_at: 1_670_000_000,
        path: "/images/design/logo.png",
        phash: "0123456789abcdee",
        relative_path: "design/logo.png",
        scene_clip_url: null,
        scene_end_frame: null,
        scene_end_seconds: null,
        scene_index: null,
        scene_start_frame: null,
        scene_start_seconds: null,
        size_bytes: 262_144,
        source_type: "local",
        source_uri: null,
        thumbnail_url: "/thumbnails/logo.png",
        width: 512,
      },
      near_duplicate: true,
      query_scene_index: null,
      vector_score: 0.7123,
    },
  ],
};

const pdfSearchResponse = {
  count: 1,
  query_audio_analysis: null,
  query_media_kind: "pdf",
  query_ocr_text: "invoice",
  query_phash: "1111111111111111",
  results: [
    {
      hash_distance: 4,
      image: {
        animated_thumbnail_url: null,
        audio_analysis: null,
        duration_ms: null,
        filename: "invoice.pdf page 001",
        frame_count: 1,
        full_audio_url: null,
        full_pdf_url: "/uploads/source-pdfs/invoice.pdf",
        full_video_url: null,
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
        scene_clip_url: null,
        scene_end_frame: null,
        scene_end_seconds: null,
        scene_index: null,
        scene_start_frame: null,
        scene_start_seconds: null,
        size_bytes: 131_072,
        source_type: "local",
        source_uri: "/documents",
        thumbnail_url: "/thumbnails/pdf-page-1.png",
        width: 900,
      },
      near_duplicate: true,
      ocr_score: 1,
      query_scene_index: 0,
      vector_score: 0.91,
    },
  ],
  scenes: [
    {
      clip_url: null,
      count: 1,
      end_frame: 1,
      end_seconds: 0,
      page_index: 0,
      page_label: "Page 1",
      page_number: 1,
      query_phash: "1111111111111111",
      results: [],
      scene_index: 0,
      scene_kind: "pdf_page",
      speaker_id: null,
      speaker_label: null,
      start_frame: 1,
      start_seconds: 0,
    },
  ],
};
(pdfSearchResponse.scenes[0].results as unknown[]).push(...pdfSearchResponse.results);

test.beforeEach(async ({ page }) => {
  await installDefaultApiMocks(page);
});

async function mockSearchResponse(page: Page, response: typeof searchResponse) {
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    await route.fulfill({ json: response });
  });
}

async function resetApiMocks(
  page: Page,
  options: Parameters<typeof installDefaultApiMocks>[1] = {},
) {
  for (const pattern of [
    "**/api/health",
    "**/api/index",
    "**/api/jobs/index",
    "**/api/jobs",
    "**/api/jobs/*/events",
    "**/api/jobs/*/cancel",
    "**/api/models",
    "**/api/models/*/download",
    "**/api/models/*/enable",
    "**/api/indexed-media/*",
    "**/api/source-config",
    "**/api/search?**",
    "**/thumbnails/**",
  ]) {
    await page.unroute(pattern).catch(() => undefined);
  }

  return installDefaultApiMocks(page, options);
}

async function uploadAndSearch(page: Page, name = "query.png") {
  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name,
  });
  await page.getByRole("button", { name: "Search" }).click();
}

async function expectResultOrder(page: Page, filenames: string[]) {
  await expect(page.locator("article h3")).toHaveText(filenames);
}

function resultCard(page: Page, filename: string) {
  return page.locator("article").filter({ has: page.getByRole("heading", { name: filename }) });
}

test("renders service health and empty UI state", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Image Similarity Service" })).toBeVisible();
  await expect(page.getByText("OK")).toBeVisible();
  await expect(page.getByText("Sources: /images, /archive")).toBeVisible();
  await expect(page.getByText("No query media selected")).toBeVisible();
  await expect(page.getByText("Metadata filters")).toBeHidden();
  await expect(
    page.getByText("Choose a query image, video, audio, or PDF and run a search."),
  ).toBeVisible();
});

test("handles service and search API failures", async ({ page }) => {
  await mockEndpointFailure(page, "**/api/health", 503, "service unavailable");
  await mockEndpointFailure(page, "**/api/search?**", 500, "search failed");
  await page.goto("/");

  await expect(page.getByText("Sources: Service is not responding")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Image Similarity Service" })).toBeVisible();

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("search failed")).toBeVisible();
});

test("navigates between web UI pages with pressed tab state", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Open query page" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await page.getByRole("button", { name: "Open inverse index" }).click();
  await expect(page.getByRole("heading", { name: "Inverse Index" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open inverse index" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await page.getByRole("button", { name: "Open media configuration" }).click();
  await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open media configuration" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await page.getByRole("button", { name: "Open indexing configuration" }).click();
  await expect(page.getByRole("heading", { name: "Indexing Configuration" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open indexing configuration" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await page.getByRole("button", { name: "Open query page" }).click();
  await expect(page.getByRole("heading", { name: "Results" })).toBeVisible();
});

test("renders inverse index people and speaker registries", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Open inverse index" }).click();

  await expect(page.getByRole("heading", { name: "Inverse Index" })).toBeVisible();
  await expect(page.getByText("Indexed media", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Depicted People" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Recognized Speakers" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Ada" })).toBeVisible();
  await expect(page.getByText("person-0001")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Voice 1" })).toBeVisible();
  await expect(page.getByText("voice-0001")).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "interview.mp3" })).toBeVisible();
  await expect(page.getByText("1.0s-8.0s")).toBeVisible();
});

test("shows active job progress and cancels running jobs", async ({ page }) => {
  const runningJob = makeJob({
    finished_at: null,
    progress: {
      completed: 3,
      message: "indexed 3/10 pending source files",
      total: 10,
      unit: "files",
    },
    spec: {
      id: "index.running.mock",
      name: "Index media sources",
    },
    status: "Running",
  });
  const mocks = await resetApiMocks(page, {
    jobEvents: makeJobEvents(runningJob),
    jobs: [runningJob],
  });
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Index Sources" }).first()).toBeDisabled();
  await expect(page.getByText("indexed 3/10 pending source files").first()).toBeVisible();
  await expect(page.getByText("30%")).toBeVisible();

  await page.getByRole("button", { name: "Cancel" }).click();

  await expect.poll(() => mocks.cancelledJobIds).toEqual(["index.running.mock"]);
  await expect(page.getByText("indexed 3/10 pending source files").first()).toBeVisible();
});

test("indexes sources from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Index Sources" }).click();

  await expect(
    page.getByText("Indexed 3 media item(s), skipped 1, pruned 1, failed 0."),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Background Jobs" })).toBeVisible();
  await expect(page.getByText("indexing complete: 3 media item(s)")).toBeVisible();
});

test("configures media sources from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();

  await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
  await expect(page.getByText("Stored in config/media-sources.txt")).toBeVisible();
  await expect(page.locator('input[value="/images"]')).toBeVisible();
  await expect(page.getByRole("heading", { name: "Local folder" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "MinIO bucket" })).toBeVisible();
  await expect(page.getByRole("option", { name: "MinIO bucket (planned)" }).first()).toBeDisabled();
  await expect(page.getByText("Images", { exact: true })).toBeVisible();
  await expect(page.getByText("PDF", { exact: true })).toBeVisible();
  await expect(page.getByText("PDF page cap", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Add Source" }).click();
  await page.getByLabel("Source spec").last().fill("/new-media");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("Saved source configuration.")).toBeVisible();
  await expect(page.getByText("/new-media")).toBeVisible();

  await page.getByRole("button", { name: "Index Sources" }).last().click();
  await expect(
    page.getByText("Indexed 3 media item(s), skipped 1, pruned 1, failed 0."),
  ).toBeVisible();
});

test("renders model status from the source configuration panel", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();

  await expect(page.getByRole("heading", { name: "Model Status" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Visual embedding" })).toBeVisible();
  await expect(page.getByTitle("xenova-clip-vit-base-patch32-onnx")).toBeVisible();
  await expect(page.getByText("Audio transcription")).toBeVisible();
});

test("configures indexing behavior from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Open indexing configuration" }).click();

  await expect(page.getByRole("heading", { name: "Indexing Configuration" })).toBeVisible();
  await expect(page.getByLabel("Image extensions")).toHaveValue(".jpg, .png, .gif");
  await expect(page.getByLabel("OCR", { exact: true })).toBeChecked();
  await expect(page.getByLabel("Audio transcription")).not.toBeChecked();
  await expect(page.getByText("Collection")).toBeVisible();
  await expect(page.getByText("image_similarity_test")).toBeVisible();

  await page.getByLabel("Image extensions").fill(".jpg, .png, webp");
  await page.getByLabel("Video frame stride").fill("12");
  await page.getByLabel("GIF motion weight").fill("0.35");
  await page.getByLabel("OCR", { exact: true }).uncheck();
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("Saved indexing configuration.")).toBeVisible();
  await expect(page.getByLabel("Image extensions")).toHaveValue(".jpg, .png, .webp");
  await expect(page.getByLabel("Video frame stride")).toHaveValue("12");
  await expect(page.getByLabel("OCR", { exact: true })).not.toBeChecked();

  await page.getByRole("button", { name: "Index Sources" }).last().click();
  await expect(
    page.getByText("Indexed 3 media item(s), skipped 1, pruned 1, failed 0."),
  ).toBeVisible();
});

test("uploads query media and renders search results", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await expect(page.getByRole("button", { name: "Search" })).toBeEnabled();
  await expect(page.getByText("Metadata filters")).toBeVisible();

  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("2 of 2 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
  await expect(page.getByText("Near duplicate", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("complementary").getByRole("button", { name: /query\.png/ }),
  ).toBeVisible();
});

test("deletes a search result from the index", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");
  await uploadAndSearch(page);

  page.on("dialog", (dialog) => dialog.accept());
  await resultCard(page, "sunrise.jpg")
    .getByRole("button", { name: /Delete sunrise\.jpg/ })
    .click();

  await expect.poll(() => mocks.deletedMediaIds).toEqual(["local-sunrise"]);
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toHaveCount(0);
});

test("keeps search disabled until media is selected and clears the selected media", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Search" })).toBeDisabled();

  await page.locator("#query-image").setInputFiles(imageUpload);
  await expect(page.getByRole("button", { name: "Search" })).toBeEnabled();
  await expect(page.getByText("Metadata filters")).toBeVisible();

  await page.getByRole("button", { name: "Clear selected media" }).click();

  await expect(page.getByRole("button", { name: "Search" })).toBeDisabled();
  await expect(page.getByText("Metadata filters")).toBeHidden();
  await expect(page.getByText("No query media selected")).toBeVisible();
});

test("handles pending search, empty results, and search errors", async ({ page }) => {
  let resumeSearch: (() => void) | null = null;
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    await new Promise<void>((resolve) => {
      resumeSearch = resolve;
    });
    await route.fulfill({
      json: makeSearchResponse({
        count: 0,
        results: [],
      }),
    });
  });
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("Searching indexed media.").first()).toBeVisible();
  await expect(page.getByLabel("Loading search results")).toBeVisible();

  resumeSearch?.();

  await expect(page.getByText("0 of 0 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByText("No indexed media matched this query.")).toBeVisible();

  await mockEndpointFailure(page, "**/api/search?**", 500, "search failed after retry");
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("search failed after retry")).toBeVisible();
});

test("renders PDF query pages and PDF result metadata", async ({ page }) => {
  await mockSearchResponse(page, pdfSearchResponse);
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: Buffer.from("%PDF-1.4\n"),
    mimeType: "application/pdf",
    name: "query.pdf",
  });
  await expect(page.getByText("PDF query selected")).toBeVisible();

  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByRole("button", { name: "Page 1" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "invoice.pdf page 001" })).toBeVisible();
  await expect(page.locator("span").filter({ hasText: "PDF page" })).toBeVisible();
  await expect(page.getByText("Page 1 of 2")).toBeVisible();
  await expect(page.getByRole("link", { name: "Open PDF" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Open page" })).toBeVisible();
});

test("renders media-specific result cards", async ({ page }) => {
  const mediaSearchResponse = makeSearchResponse({
    count: 5,
    results: [
      makeResult({
        image: {
          animated_thumbnail_url: "/thumbnails/dance-animated.gif",
          filename: "dance.gif",
          frame_count: 12,
          id: "gif-dance",
          media_kind: "animated_gif",
          relative_path: "gifs/dance.gif",
          thumbnail_url: "/thumbnails/dance-still.png",
        },
        near_duplicate: false,
      }),
      makeResult({
        hash_distance: null,
        image: {
          filename: "clip.mp4",
          full_video_url: "/media/clip.mp4",
          height: 1080,
          id: "video-clip",
          media_kind: "video_scene",
          relative_path: "clips/clip.mp4",
          scene_clip_url: "/clips/clip-scene.mp4",
          scene_end_frame: 240,
          scene_end_seconds: 10,
          scene_start_frame: 120,
          scene_start_seconds: 5,
          thumbnail_url: "/thumbnails/clip.png",
          width: 1920,
        },
        near_duplicate: false,
        vector_score: 0.99,
      }),
      makeResult({
        image: {
          audio_analysis: {
            audio_segments: [
              {
                confidence: 0.9,
                end_seconds: 4,
                kind: "speech",
                segment_index: 0,
                speaker_id: "voice-alice",
                speaker_label: "Alice",
                start_seconds: 1,
              },
            ],
            recognized_voices: [
              {
                confidence: 0.93,
                id: "voice-alice",
                label: "Alice",
                segment_count: 1,
                total_seconds: 3,
              },
            ],
            speech_detected: true,
            speech_ratio: 0.75,
            speech_segments: [],
            tempo_bpm: 128.4,
            tempo_confidence: 0.82,
            tempo_onset_count: 12,
            transcript_language: "en",
            transcript_segments: [],
            transcript_text: "hello indexed audio",
          },
          duration_ms: 4000,
          filename: "voice.mp3",
          full_audio_url: "/media/voice.mp3",
          id: "audio-voice",
          media_kind: "audio",
          relative_path: "audio/voice.mp3",
          scene_end_seconds: 4,
          scene_start_seconds: 1,
          thumbnail_url: null,
        },
        near_duplicate: false,
      }),
      makeResult({
        image: {
          faces: [
            {
              bbox: { height: 80, width: 80, x: 10, y: 10 },
              confidence: 0.98,
              face_id: "face-1",
              frame_index: 0,
              media_id: "ocr-face",
              person_id: "person-1",
              person_label: "Ada",
            },
          ],
          filename: "ocr-face.png",
          id: "ocr-face",
          ocr_text: "Conference badge",
          people: [
            {
              confidence: 0.95,
              face_count: 2,
              label: "Ada",
              media_count: 1,
              person_id: "person-1",
            },
          ],
          relative_path: "people/ocr-face.png",
        },
        near_duplicate: false,
        ocr_score: 0.88,
      }),
      makeResult({
        image: {
          filename: "missing-thumb.png",
          id: "missing-thumb",
          relative_path: "missing-thumb.png",
          thumbnail_url: null,
        },
        near_duplicate: false,
      }),
    ],
  });
  await mockSearchResponseRoute(page, mediaSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await expect(resultCard(page, "dance.gif").getByText("GIF", { exact: true })).toBeVisible();
  await expect(resultCard(page, "dance.gif").locator("img")).toHaveAttribute(
    "src",
    "/thumbnails/dance-animated.gif",
  );
  await expect(resultCard(page, "clip.mp4").getByText("Video scene")).toBeVisible();
  await expect(resultCard(page, "clip.mp4").getByText("5.0s-10.0s · frames 120-240")).toBeVisible();
  await expect(
    resultCard(page, "clip.mp4").getByRole("link", { name: "Full video" }),
  ).toBeVisible();
  await expect(
    resultCard(page, "clip.mp4").getByRole("link", { name: "Scene clip" }),
  ).toBeVisible();
  await expect(resultCard(page, "voice.mp3").locator("audio")).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3")
      .locator("span")
      .filter({ hasText: /^Speech$/ }),
  ).toBeVisible();
  await expect(resultCard(page, "voice.mp3").getByText("hello indexed audio")).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3")
      .locator("span")
      .filter({ hasText: /^Alice$/ }),
  ).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3")
      .locator("span")
      .filter({ hasText: /^128 BPM$/ }),
  ).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3").getByRole("link", { name: "Open audio" }),
  ).toBeVisible();
  await expect(resultCard(page, "ocr-face.png").getByText("OCR score")).toBeVisible();
  await expect(resultCard(page, "ocr-face.png").getByText("Conference badge")).toBeVisible();
  await expect(resultCard(page, "ocr-face.png").getByText("Faces 1")).toBeVisible();
  await expect(
    resultCard(page, "ocr-face.png").locator("span").filter({ hasText: /^Ada$/ }),
  ).toBeVisible();
  await expect(resultCard(page, "missing-thumb.png").getByText("Dimensions")).toBeVisible();
  await expect(resultCard(page, "missing-thumb.png").locator("img")).toHaveCount(0);
});

test("metadata-filtered searches request a wider candidate set", async ({ page }) => {
  let requestedLimit: string | null = null;
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    requestedLimit = new URL(route.request().url()).searchParams.get("limit");
    await route.fulfill({ json: searchResponse });
  });
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await page.getByLabel("Result limit").fill("1");
  await page.getByLabel("Media type").selectOption("static_image");
  await page.getByRole("button", { name: "Search" }).click();

  await expect.poll(() => requestedLimit).toBe("8");
  await expect(page.locator("article h3")).toHaveCount(1);
});

test("sorts results by pHash distance by default and supports changing sort", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.locator("article h3")).toHaveText(["sunrise.jpg", "portrait.png"]);

  await page.getByLabel("Sort").selectOption("vector_score");

  await expect(page.locator("article h3")).toHaveText(["portrait.png", "sunrise.jpg"]);
});

test("sorts rendered results with every supported sort mode", async ({ page }) => {
  await mockSearchResponse(page, sortableSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await expectResultOrder(page, ["logo.png", "sunrise.jpg", "portrait.png", "clip.mp4"]);

  await page.getByLabel("Sort").selectOption("vector_score");
  await expectResultOrder(page, ["clip.mp4", "portrait.png", "logo.png", "sunrise.jpg"]);

  await page.getByLabel("Sort").selectOption("modified_newest");
  await expectResultOrder(page, ["clip.mp4", "sunrise.jpg", "portrait.png", "logo.png"]);

  await page.getByLabel("Sort").selectOption("size_largest");
  await expectResultOrder(page, ["clip.mp4", "portrait.png", "sunrise.jpg", "logo.png"]);

  await page.getByLabel("Sort").selectOption("filename");
  await expectResultOrder(page, ["clip.mp4", "logo.png", "portrait.png", "sunrise.jpg"]);

  await page.getByLabel("Sort").selectOption("phash_distance");
  await expectResultOrder(page, ["logo.png", "sunrise.jpg", "portrait.png", "clip.mp4"]);
});

test("renders and switches video query scenes", async ({ page }) => {
  const sceneOneResult = makeResult({
    image: {
      filename: "first-scene-match.jpg",
      id: "first-scene-match",
      relative_path: "scenes/first.jpg",
    },
  });
  const sceneTwoResult = makeResult({
    image: {
      filename: "second-scene-match.jpg",
      id: "second-scene-match",
      relative_path: "scenes/second.jpg",
    },
  });
  await mockSearchResponseRoute(
    page,
    makeSearchResponse({
      count: 2,
      query_media_kind: "video",
      results: [sceneOneResult, sceneTwoResult],
      scenes: [
        makeScene({
          count: 1,
          end_frame: 48,
          end_seconds: 2,
          results: [sceneOneResult],
          scene_index: 0,
          scene_kind: "video_scene",
          start_frame: 24,
          start_seconds: 1,
        }),
        makeScene({
          clip_url: "/clips/query-scene-2.mp4",
          count: 1,
          end_frame: 96,
          end_seconds: 4,
          results: [sceneTwoResult],
          scene_index: 1,
          scene_kind: "video_scene",
          start_frame: 72,
          start_seconds: 3,
        }),
      ],
    }),
  );
  await page.goto("/");

  await uploadAndSearch(page);

  await expect(page.getByRole("button", { name: /Scene 1 .*1\.0s-2\.0s/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Scene 2 .*3\.0s-4\.0s/ })).toBeVisible();
  await expectResultOrder(page, ["first-scene-match.jpg"]);

  await page.getByRole("button", { name: /Scene 2/ }).click();

  await expect(page.getByRole("link", { name: "Open query clip" })).toBeVisible();
  await expectResultOrder(page, ["second-scene-match.jpg"]);
});

test("renders and switches audio query bits", async ({ page }) => {
  const aliceResult = makeResult({
    image: {
      filename: "alice-voice.mp3",
      id: "alice-voice",
      media_kind: "audio",
      relative_path: "audio/alice.mp3",
    },
  });
  const bobResult = makeResult({
    image: {
      filename: "bob-voice.mp3",
      id: "bob-voice",
      media_kind: "audio",
      relative_path: "audio/bob.mp3",
    },
  });
  await mockSearchResponseRoute(
    page,
    makeSearchResponse({
      count: 2,
      query_media_kind: "audio",
      results: [aliceResult, bobResult],
      scenes: [
        makeScene({
          count: 1,
          end_seconds: 2,
          results: [aliceResult],
          scene_index: 0,
          scene_kind: "audio_bit",
          speaker_id: "voice-alice",
          speaker_label: "Alice",
          start_seconds: 0,
        }),
        makeScene({
          count: 1,
          end_seconds: 5,
          results: [bobResult],
          scene_index: 1,
          scene_kind: "audio_bit",
          speaker_id: "voice-bob",
          speaker_label: "Bob",
          start_seconds: 3,
        }),
      ],
    }),
  );
  await page.goto("/");

  await uploadAndSearch(page);

  await expect(page.getByRole("button", { name: /Bit 1 .* Alice/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Bit 2 .* Bob/ })).toBeVisible();
  await expectResultOrder(page, ["alice-voice.mp3"]);

  await page.getByRole("button", { name: /Bit 2/ }).click();

  await expect(page.getByText("3.0s-5.0s · Bob", { exact: true })).toBeVisible();
  await expectResultOrder(page, ["bob-voice.mp3"]);
});

test("filters rendered results by filename", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await page.getByRole("button", { name: "Search" }).click();
  await page.getByLabel("Name or path").fill("sunrise");

  await expect(page.getByText("1 of 2 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeHidden();

  await page.getByRole("button", { name: "Clear 1" }).click();

  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
});

test("filters rendered results by metadata fields", async ({ page }) => {
  await mockSearchResponse(page, sortableSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await page.getByLabel("Source type").selectOption("import");
  await expect(page.getByText("2 of 4 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expectResultOrder(page, ["portrait.png", "clip.mp4"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Duplicate status").selectOption("only");
  await expectResultOrder(page, ["logo.png", "sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Orientation").selectOption("portrait");
  await expectResultOrder(page, ["portrait.png"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Min file size (MB)").fill("2");
  await expectResultOrder(page, ["portrait.png", "clip.mp4"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Media type").selectOption("video_scene");
  await expectResultOrder(page, ["clip.mp4"]);
});

test("applies date, dimension, person, max-size, and duplicate-exclusion filters", async ({
  page,
}) => {
  const filteredSearchResponse = makeSearchResponse({
    count: 4,
    results: [
      makeResult({
        hash_distance: 1,
        image: {
          filename: "alice-landscape.jpg",
          height: 800,
          id: "alice-landscape",
          modified_at: Date.parse("2024-01-10T00:00:00Z") / 1000,
          people: [
            {
              confidence: 0.91,
              face_count: 1,
              label: "Alice",
              media_count: 1,
              person_id: "person-1",
            },
          ],
          relative_path: "people/alice-landscape.jpg",
          size_bytes: 1_500_000,
          width: 1200,
        },
        near_duplicate: false,
      }),
      makeResult({
        hash_distance: 2,
        image: {
          filename: "duplicate-square.jpg",
          height: 512,
          id: "duplicate-square",
          modified_at: Date.parse("2024-02-15T00:00:00Z") / 1000,
          relative_path: "dupes/duplicate-square.jpg",
          size_bytes: 500_000,
          width: 512,
        },
        near_duplicate: true,
      }),
      makeResult({
        hash_distance: 3,
        image: {
          filename: "wide-large.jpg",
          height: 900,
          id: "wide-large",
          modified_at: Date.parse("2024-03-20T00:00:00Z") / 1000,
          relative_path: "wide-large.jpg",
          size_bytes: 6_000_000,
          width: 1600,
        },
        near_duplicate: false,
      }),
      makeResult({
        hash_distance: 4,
        image: {
          filename: "old-portrait.jpg",
          height: 1400,
          id: "old-portrait",
          modified_at: Date.parse("2023-11-01T00:00:00Z") / 1000,
          relative_path: "old-portrait.jpg",
          size_bytes: 2_000_000,
          width: 900,
        },
        near_duplicate: false,
      }),
    ],
  });
  await mockSearchResponseRoute(page, filteredSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await page.getByLabel("Modified after").fill("2024-01-01");
  await page.getByLabel("Modified before").fill("2024-02-29");
  await expectResultOrder(page, ["alice-landscape.jpg", "duplicate-square.jpg"]);

  await page.getByRole("button", { name: "Clear 2" }).click();
  await page.getByLabel("Minimum width").fill("1000");
  await page.getByLabel("Maximum height").fill("900");
  await expectResultOrder(page, ["alice-landscape.jpg", "wide-large.jpg"]);

  await page.getByRole("button", { name: "Clear 2" }).click();
  await page.getByLabel("Person ID").fill("person-1");
  await expectResultOrder(page, ["alice-landscape.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Duplicate status").selectOption("exclude");
  await expectResultOrder(page, ["alice-landscape.jpg", "wide-large.jpg", "old-portrait.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Max file size (MB)").fill("1");
  await expectResultOrder(page, ["duplicate-square.jpg"]);

  await page.getByLabel("Name or path").fill("duplicate");
  await expect(page.getByRole("button", { name: "Clear 2" })).toBeVisible();
  await page.getByRole("button", { name: "Clear 2" }).click();
  await expect(page.getByLabel("Name or path")).toHaveValue("");
  await expect(page.getByLabel("Max file size (MB)")).toHaveValue("");
  await expectResultOrder(page, [
    "alice-landscape.jpg",
    "duplicate-square.jpg",
    "wide-large.jpg",
    "old-portrait.jpg",
  ]);
});

test("displays and filters photo metadata", async ({ page }) => {
  const photoSearchResponse = makeSearchResponse({
    count: 3,
    results: [
      makeResult({
        hash_distance: 1,
        image: {
          filename: "camera-sunrise.jpg",
          id: "camera-sunrise",
          photo_metadata: {
            camera_make: "Acme",
            camera_model: "Pocket 7",
            capture_time: "2024-03-12T10:30:00Z",
            copyright: "Acme Studio",
            creator: "Mira",
            description: "Morning ridge",
            gps: { altitude_meters: 42.4, latitude: 52.5, longitude: 13.4 },
            keywords: ["Travel", "Sunrise"],
            lens_model: "35mm Prime",
            orientation: "Horizontal",
            rating: 4,
            raw: [
              {
                key: "CreateDate",
                label: "CreateDate",
                namespace: "xmp",
                value: "2024-03-12T10:30:00Z",
              },
            ],
            title: "Camera sunrise",
          },
          relative_path: "photos/camera-sunrise.jpg",
        },
      }),
      makeResult({
        hash_distance: 2,
        image: {
          filename: "older-canon.jpg",
          id: "older-canon",
          photo_metadata: {
            camera_make: "Canon",
            camera_model: "R6",
            capture_time: "2023-01-05T08:00:00Z",
            copyright: null,
            creator: null,
            description: null,
            gps: null,
            keywords: ["Portrait"],
            lens_model: "85mm",
            orientation: null,
            rating: null,
            raw: [],
            title: null,
          },
          relative_path: "photos/older-canon.jpg",
        },
      }),
      makeResult({
        hash_distance: 3,
        image: {
          filename: "missing-metadata.jpg",
          id: "missing-metadata",
          photo_metadata: null,
          relative_path: "photos/missing-metadata.jpg",
        },
      }),
    ],
  });
  await mockSearchResponseRoute(page, photoSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  const card = resultCard(page, "camera-sunrise.jpg");
  await expect(card.getByText("Captured")).toBeVisible();
  await expect(card.getByText("Acme Pocket 7")).toBeVisible();
  await expect(card.getByText("35mm Prime")).toBeVisible();
  await expect(card.getByText("52.50000, 13.40000, 42.4 m")).toBeVisible();
  await expect(card.getByText("Travel, Sunrise")).toBeVisible();
  await card.getByText("Photo metadata").click();
  await expect(card.getByText("xmp · CreateDate")).toBeVisible();

  await page.getByLabel("Camera/lens").fill("pocket");
  await expectResultOrder(page, ["camera-sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Keyword").fill("portrait");
  await expectResultOrder(page, ["older-canon.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("GPS metadata").selectOption("yes");
  await expectResultOrder(page, ["camera-sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Captured after").fill("2024-01-01");
  await expectResultOrder(page, ["camera-sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Sort").selectOption("captured_newest");
  await expectResultOrder(page, ["camera-sunrise.jpg", "older-canon.jpg", "missing-metadata.jpg"]);
});

test("sends OCR and person search parameters and caps filtered candidate limit", async ({
  page,
}) => {
  const personSearchResponse = makeSearchResponse({
    results: [
      makeResult({
        image: {
          filename: "person-match.jpg",
          id: "person-match",
          people: [
            {
              confidence: 0.94,
              face_count: 2,
              label: "Person One",
              media_count: 1,
              person_id: "person-1",
            },
          ],
          relative_path: "people/person-match.jpg",
        },
      }),
      makeResult({
        image: {
          filename: "other-person.jpg",
          id: "other-person",
          people: [
            {
              confidence: 0.94,
              face_count: 2,
              label: "Person Two",
              media_count: 1,
              person_id: "person-2",
            },
          ],
          relative_path: "people/other-person.jpg",
        },
      }),
    ],
  });
  const capture = await captureSearchRequests(page, personSearchResponse);
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByLabel("Result limit").fill("100");
  await page.getByLabel("Text in media").fill("invoice");
  await page.getByLabel("Person ID").fill("person-1");
  await page.getByRole("button", { name: "Search" }).click();

  await expect.poll(() => capture.count).toBe(1);
  await expect
    .poll(() => capture.requests[0])
    .toEqual({
      limit: "500",
      ocrText: "invoice",
      personId: "person-1",
    });
  await expectResultOrder(page, ["person-match.jpg"]);
});

test("handles all query media preview types", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await expect(page.getByAltText("Query preview")).toBeVisible();

  await page.getByRole("button", { name: "Clear selected media" }).click();
  await page.locator("#query-image").setInputFiles(gifUpload);
  await expect(page.getByAltText("Query preview")).toBeVisible();
  await page.getByRole("button", { name: "Search" }).click();
  await expect(
    page.getByRole("complementary").getByRole("button", { name: /query\.gif/ }),
  ).toBeVisible();

  await page.locator("#query-image").setInputFiles(videoUpload);
  await expect(page.locator("video[controls]")).toBeVisible();

  await page.locator("#query-image").setInputFiles(audioUpload);
  await expect(page.locator("audio[controls]")).toBeVisible();

  await page.locator("#query-image").setInputFiles(pdfUpload);
  await expect(page.getByText("PDF query selected")).toBeVisible();
});

test("keeps search history bounded to the newest eight entries", async ({ page }) => {
  await page.goto("/");

  for (let index = 1; index <= 9; index += 1) {
    const name = `query-${index}.png`;
    await page.locator("#query-image").setInputFiles({
      ...imageUpload,
      name,
    });
    await page.getByRole("button", { name: "Search" }).click();
    await expect(
      page.getByRole("complementary").getByRole("button", { name: new RegExp(name) }),
    ).toBeVisible();
  }

  const historyButtons = page.getByRole("complementary").getByRole("button");
  await expect(historyButtons).toHaveCount(8);
  await expect(
    page.getByRole("complementary").getByRole("button", { name: /query-1\.png/ }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("complementary").getByRole("button", { name: /query-9\.png/ }),
  ).toBeVisible();
});

test("ignores corrupt search history localStorage", async ({ page }) => {
  await page.addInitScript((key) => {
    localStorage.setItem(key, "{not json");
  }, historyStorageKey);

  await page.goto("/");

  await expect(page.getByText("No searches yet.")).toBeVisible();
});

test("normalizes stored blob preview URLs from search history", async ({ page }) => {
  const historyItem = {
    fileName: "blob-query.png",
    filters: {},
    id: "blob-history",
    limit: 12,
    queryImageUrl: "blob:http://127.0.0.1/not-restorable",
    queryMediaKind: "static_image",
    response: fixtureSearchResponse,
    searchedAt: "2026-05-20T10:00:00.000Z",
  };
  await page.addInitScript(
    ([key, history]) => {
      localStorage.setItem(key, JSON.stringify(history));
    },
    [historyStorageKey, [historyItem]],
  );
  await page.goto("/");

  await page
    .getByRole("complementary")
    .getByRole("button", { name: /blob-query\.png/ })
    .click();

  await expect(page.getByAltText("Query preview")).toHaveCount(0);
  await expect(page.getByText("No query media selected")).toBeVisible();
});

test("covers source editing edge cases", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();
  await page.getByRole("button", { name: "Remove source 1" }).click();
  await page.getByRole("button", { name: "Remove source 1" }).click();

  await expect(page.getByText("No media sources configured.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();

  await page.getByRole("button", { name: "Add Source" }).click();
  await page.getByLabel("Source 1", { exact: true }).selectOption("custom");
  await page.getByLabel("Source spec").fill("custom://media");
  await page.getByRole("button", { name: "Save" }).click();

  await expect.poll(() => mocks.sourceConfigPuts.length).toBe(1);
  await expect
    .poll(() => mocks.sourceConfigPuts[0])
    .toEqual({
      sources: ["custom://media"],
    });
  await expect(page.getByText("Saved source configuration.")).toBeVisible();
});

test("renders source save failures and non-ready source statuses", async ({ page }) => {
  await resetApiMocks(page, {
    sourceConfig: makeSourceConfigResponse({
      sources: [
        {
          detail: "Folder does not exist",
          kind: "local",
          spec: "/missing",
          status: "unavailable",
        },
        {
          detail: "Scheme is not supported",
          kind: "ftp",
          spec: "ftp://media",
          status: "unsupported",
        },
      ],
    }),
  });
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();
  await expect(page.getByText("unavailable")).toBeVisible();
  await expect(page.getByText("Folder does not exist")).toBeVisible();
  await expect(page.getByText("unsupported")).toBeVisible();
  await expect(page.getByText("Scheme is not supported")).toBeVisible();

  await page.unroute("**/api/source-config");
  await page.route("**/api/source-config", async (route) => {
    if (route.request().method() === "PUT") {
      await route.fulfill({
        json: { detail: "source save failed" },
        status: 500,
      });
      return;
    }

    await route.fulfill({ json: sourceConfigResponse });
  });
  await page.getByLabel("Source spec").first().fill("/still-missing");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("source save failed")).toBeVisible();
  await expect(page.getByText("Saved source configuration.")).toHaveCount(0);
});

test("covers indexing configuration edge cases", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Open indexing configuration" }).click();
  await page.getByLabel("Image extensions").fill("");
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();

  await page.getByLabel("Image extensions").fill(".JPG, png, .jpg");
  await page.getByLabel("Video max frames").fill("");
  await page.getByRole("button", { name: "Save" }).click();

  await expect.poll(() => mocks.indexingConfigPuts.length).toBe(1);
  await expect
    .poll(() => mocks.indexingConfigPuts[0])
    .toMatchObject({
      indexing: {
        image_extensions: [".jpg", ".png"],
        video_max_frames: null,
      },
    });
  await expect(page.getByLabel("Image extensions")).toHaveValue(".jpg, .png");
  await expect(page.getByLabel("Video max frames")).toHaveValue("");

  await page.getByLabel("Video max frames").fill("24");
  await page.getByLabel("PDF render DPI").fill("180");
  await page.getByLabel("OCR frames").fill("8");
  await page.getByLabel("Face confidence").fill("0.85");
  await page.getByLabel("GIF preview frames").fill("20");
  await page.getByRole("button", { name: "Save" }).click();

  await expect.poll(() => mocks.indexingConfigPuts.length).toBe(2);
  await expect
    .poll(() => mocks.indexingConfigPuts[1])
    .toMatchObject({
      indexing: {
        face_detection_min_confidence: 0.85,
        gif_preview_frames: 20,
        ocr_max_frames: 8,
        pdf_render_dpi: 180,
        video_max_frames: 24,
      },
    });
  await expect(page.getByLabel("Video max frames")).toHaveValue("24");
});

test("renders indexing configuration save failures", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open indexing configuration" }).click();

  await page.unroute("**/api/source-config");
  await page.route("**/api/source-config", async (route) => {
    if (route.request().method() === "PUT") {
      await route.fulfill({
        json: { detail: "indexing save failed" },
        status: 500,
      });
      return;
    }

    await route.fulfill({ json: sourceConfigResponse });
  });
  await page.getByLabel("Video frame stride").fill("9");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("indexing save failed")).toBeVisible();
  await expect(page.getByText("Saved indexing configuration.")).toHaveCount(0);
});

test("restores filters and sort mode from search history", async ({ page }) => {
  await mockSearchResponse(page, sortableSearchResponse);
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "first-query.png",
  });
  await page.getByLabel("Name or path").fill("sun");
  await page.getByLabel("Sort").selectOption("vector_score");
  await page.getByRole("button", { name: "Search" }).click();
  await expectResultOrder(page, ["sunrise.jpg"]);

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "second-query.png",
  });
  await page.getByLabel("Name or path").fill("");
  await page.getByLabel("Sort").selectOption("filename");
  await page.getByRole("button", { name: "Search" }).click();
  await expectResultOrder(page, ["clip.mp4", "logo.png", "portrait.png", "sunrise.jpg"]);

  await page
    .getByRole("complementary")
    .getByRole("button", { name: /first-query\.png/ })
    .click();

  await expect(page.getByLabel("Name or path")).toHaveValue("sun");
  await expect(page.getByLabel("Sort")).toHaveValue("vector_score");
  await expectResultOrder(page, ["sunrise.jpg"]);
});

test("updates filters and sort mode on selected search history entries", async ({ page }) => {
  await mockSearchResponse(page, sortableSearchResponse);
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "first-query.png",
  });
  await page.getByLabel("Name or path").fill("sun");
  await page.getByRole("button", { name: "Search" }).click();
  await expectResultOrder(page, ["sunrise.jpg"]);

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "second-query.png",
  });
  await page.getByLabel("Name or path").fill("");
  await page.getByLabel("Sort").selectOption("filename");
  await page.getByRole("button", { name: "Search" }).click();
  await expectResultOrder(page, ["clip.mp4", "logo.png", "portrait.png", "sunrise.jpg"]);

  const historyButtons = page.getByRole("complementary").getByRole("button");
  await expect(historyButtons).toHaveCount(2);

  await page
    .getByRole("complementary")
    .getByRole("button", { name: /first-query\.png/ })
    .click();
  await page.getByLabel("Name or path").fill("portrait");
  await page.getByLabel("Sort").selectOption("size_largest");
  await expectResultOrder(page, ["portrait.png"]);

  await page
    .getByRole("complementary")
    .getByRole("button", { name: /second-query\.png/ })
    .click();
  await expect(page.getByLabel("Name or path")).toHaveValue("");
  await expect(page.getByLabel("Sort")).toHaveValue("filename");

  await page
    .getByRole("complementary")
    .getByRole("button", { name: /first-query\.png/ })
    .click();
  await expect(page.getByLabel("Name or path")).toHaveValue("portrait");
  await expect(page.getByLabel("Sort")).toHaveValue("size_largest");
  await expectResultOrder(page, ["portrait.png"]);

  await page.getByRole("button", { name: "Search" }).click();
  await expect(historyButtons).toHaveCount(3);
});

test("loads legacy search history with default sorting", async ({ page }) => {
  const legacyHistoryItem = {
    id: "legacy-search",
    fileName: "legacy-query.png",
    filters: {
      nameQuery: "clip",
    },
    limit: 12,
    queryImageUrl: null,
    queryMediaKind: "static_image",
    response: {
      count: sortableSearchResponse.count,
      query_phash: sortableSearchResponse.query_phash,
      results: sortableSearchResponse.results,
    },
    searchedAt: "2026-05-20T10:00:00.000Z",
  };

  await page.addInitScript(
    ([key, history]) => {
      localStorage.setItem(key, JSON.stringify(history));
    },
    [historyStorageKey, [legacyHistoryItem]],
  );
  await page.goto("/");

  await page
    .getByRole("complementary")
    .getByRole("button", { name: /legacy-query\.png/ })
    .click();

  await expect(page.getByLabel("Name or path")).toHaveValue("clip");
  await expect(page.getByLabel("Sort")).toHaveValue("phash_distance");
  await expectResultOrder(page, ["clip.mp4"]);
});

test.describe("mobile viewport", () => {
  test.use({ viewport: { height: 844, width: 390 } });

  test("supports core navigation and search on mobile", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Open media configuration" }).click();
    await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save" })).toBeVisible();

    await page.getByRole("button", { name: "Open indexing configuration" }).click();
    await expect(page.getByRole("heading", { name: "Indexing Configuration" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save" })).toBeVisible();

    await page.getByRole("button", { name: "Open query page" }).click();
    await page.locator("#query-image").setInputFiles(imageUpload);
    await expect(page.getByRole("button", { name: "Search" })).toBeVisible();
    await page.getByRole("button", { name: "Search" }).click();

    await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  });
});

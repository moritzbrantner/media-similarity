import { expect, test, type Page } from "@playwright/test";

const pngPixel = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=",
  "base64",
);

const healthResponse = {
  collection: "image_similarity_test",
  source_dir: "/images",
  sources: ["/images", "/archive"],
  status: "ok",
};

const indexResponse = {
  collection: "image_similarity_test",
  errors: [],
  failed: 0,
  indexed: 3,
  skipped: 1,
  source_dir: "/images",
  sources: ["/images", "/archive"],
};

const sourceConfigResponse = {
  default_source_dir: "/images",
  indexing: {
    audio_extensions: [".mp3", ".wav"],
    audio_transcription_enabled: false,
    collection: "image_similarity_test",
    gif_max_decode_frames: 512,
    gif_motion_weight: 0.2,
    gif_preview_frames: 16,
    gif_sample_frames: 16,
    image_extensions: [".jpg", ".png", ".gif"],
    ocr_enabled: true,
    ocr_max_frames: 4,
    video_extensions: [".mp4", ".mov"],
    video_frame_stride: 30,
    video_max_frames: null,
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

const historyStorageKey = "image-similarity-search-history";

test.beforeEach(async ({ page }) => {
  await page.route("**/api/health", async (route) => {
    await route.fulfill({ json: healthResponse });
  });

  await page.route("**/api/index", async (route) => {
    await route.fulfill({ json: indexResponse });
  });

  await page.route("**/api/source-config", async (route) => {
    if (route.request().method() === "PUT") {
      const request = route.request().postDataJSON() as { sources: string[] };
      await route.fulfill({
        json: {
          ...sourceConfigResponse,
          sources: request.sources.map((spec) => ({
            detail: null,
            kind: spec.startsWith("minio:") ? "minio" : "local",
            spec,
            status: spec.startsWith("minio:") ? "not_implemented" : "ready",
          })),
        },
      });
      return;
    }

    await route.fulfill({ json: sourceConfigResponse });
  });

  await page.route("**/api/search?**", async (route) => {
    await route.fulfill({ json: searchResponse });
  });

  await page.route("**/thumbnails/**", async (route) => {
    await route.fulfill({
      body: pngPixel,
      contentType: "image/png",
    });
  });
});

async function mockSearchResponse(page: Page, response: typeof searchResponse) {
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    await route.fulfill({ json: response });
  });
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

test("renders service health and empty UI state", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Image Similarity Service" })).toBeVisible();
  await expect(page.getByText("OK")).toBeVisible();
  await expect(page.getByText("Sources: /images, /archive")).toBeVisible();
  await expect(page.getByText("No query media selected")).toBeVisible();
  await expect(page.getByText("Metadata filters")).toBeHidden();
  await expect(
    page.getByText("Choose a query image, video, or audio and run a search."),
  ).toBeVisible();
});

test("indexes sources from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Index Sources" }).click();

  await expect(page.getByText("Indexed 3 media item(s), skipped 1, failed 0.")).toBeVisible();
});

test("configures media sources from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();

  await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
  await expect(page.getByText("Stored in config/media-sources.txt")).toBeVisible();
  await expect(page.locator('input[value="/images"]')).toBeVisible();
  await expect(page.getByRole("heading", { name: "Local folder" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "MinIO bucket" })).toBeVisible();
  await expect(page.getByText("Images", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Add Source" }).click();
  await page.getByLabel("Source spec").last().fill("/new-media");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("Saved source configuration.")).toBeVisible();
  await expect(page.getByText("/new-media")).toBeVisible();

  await page.getByRole("button", { name: "Index Sources" }).last().click();
  await expect(page.getByText("Indexed 3 media item(s), skipped 1, failed 0.")).toBeVisible();
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

import { expect, test } from "@playwright/test";

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

const searchResponse = {
  count: 2,
  query_audio_analysis: null,
  query_media_kind: "static_image",
  query_phash: "0123456789abcdef",
  results: [
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
      vector_score: 0.9876,
    },
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
      vector_score: 0.7123,
    },
  ],
  scenes: [],
};

test.beforeEach(async ({ page }) => {
  await page.route("**/api/health", async (route) => {
    await route.fulfill({ json: healthResponse });
  });

  await page.route("**/api/index", async (route) => {
    await route.fulfill({ json: indexResponse });
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

test("renders service health and empty UI state", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Image Similarity Service" })).toBeVisible();
  await expect(page.getByText("OK")).toBeVisible();
  await expect(page.getByText("Sources: /images, /archive")).toBeVisible();
  await expect(page.getByText("No query media selected")).toBeVisible();
  await expect(
    page.getByText("Choose a query image, video, or audio and run a search."),
  ).toBeVisible();
});

test("indexes sources from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Index Sources" }).click();

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

  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("2 of 2 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
  await expect(page.getByText("Near duplicate", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("complementary").getByRole("button", { name: /query\.png/ }),
  ).toBeVisible();
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

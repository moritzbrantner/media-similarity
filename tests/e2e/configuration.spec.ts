import { expect, test } from "@playwright/test";

import {
  captureSearchRequests,
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
  pngPixel,
  pdfSearchResponse,
  pdfUpload,
  searchResponse as fixtureSearchResponse,
  sortableSearchResponse,
  sourceConfigResponse,
  videoUpload,
} from "./support/media-fixtures";
import {
  expectResultOrder,
  installUiTestMocks,
  mockSearchResponse,
  resetApiMocks,
  resultCard,
  uploadAndSearch,
} from "./support/page-objects";

test.beforeEach(async ({ page }) => {
  await installUiTestMocks(page);
});

test("configures media sources from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();

  await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
  await expect(page.getByText("Stored in config/media-sources.txt")).toBeVisible();
  await expect(page.locator('input[value="/images"]')).toBeVisible();
  await expect(page.getByRole("heading", { name: "Local folder" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "MinIO bucket" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "S3 bucket" })).toBeVisible();
  await expect(page.getByRole("option", { name: "MinIO bucket" }).first()).toBeEnabled();
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

test("disables source saves when the source file is read-only", async ({ page }) => {
  await resetApiMocks(page, {
    sourceConfig: makeSourceConfigResponse({
      media_sources_file: "/app/data/media-sources.txt",
      media_sources_seed_file: "/app/config/media-sources.txt",
      media_sources_writable: false,
    }),
  });
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();

  await expect(page.getByText("Stored in /app/data/media-sources.txt")).toBeVisible();
  await expect(page.getByText("Seeded from /app/config/media-sources.txt")).toBeVisible();
  await expect(page.getByText("Source configuration file is not writable.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();
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

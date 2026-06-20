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
  modelsResponse,
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
    page.getByText("Indexed 3 media item(s), already indexed 2, skipped 1, pruned 1, failed 0."),
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

test("disables active models from the source configuration panel", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();
  await page.getByRole("button", { exact: true, name: "Disable" }).click();

  await expect.poll(() => mocks.modelDisables).toEqual([{ role: "visual_embedding" }]);
});

test("calls out blocking first-run models and downloads them from the panel", async ({ page }) => {
  const mocks = await resetApiMocks(page, {
    models: {
      models: [
        {
          ...modelsResponse.models[0],
          active: false,
          blocking: true,
          bundle_path: null,
          cached: false,
          detail: "Model bundle is not cached in /app/data/models/bundles",
          required_action: "download",
        },
        modelsResponse.models[1],
      ],
    },
  });
  await page.goto("/");

  await page.getByRole("button", { name: "Open media configuration" }).click();

  await expect(page.getByText("blocking")).toBeVisible();
  await expect(page.getByText("blocks indexing and search")).toBeVisible();

  await page.getByRole("button", { exact: true, name: "Download" }).first().click();

  await expect
    .poll(() => mocks.modelDownloads)
    .toEqual([
      {
        model: "xenova-clip-vit-base-patch32-onnx",
        role: "visual_embedding",
      },
    ]);
});

test("configures processing workflows from the UI", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Open workflow editor" }).click();

  await expect(page.getByRole("heading", { name: "Processing Workflows" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Workflow document" })).toHaveValue(
    "static_image",
  );
  await expect(page.getByRole("button", { exact: true, name: "Decode image" })).toBeVisible();
  await expect(page.getByText("No workflow diagnostics.")).toBeVisible();

  await page.getByRole("button", { name: "Validate" }).click();
  await expect.poll(() => mocks.workflowValidations.length).toBe(1);

  await page.getByRole("button", { name: "Save" }).click();

  await expect.poll(() => mocks.workflowPuts.length).toBe(1);
  await expect(page.getByText("Saved workflows.")).toBeVisible();

  await page.getByRole("button", { name: "Index Sources" }).last().click();
  await expect(
    page.getByText("Indexed 3 media item(s), already indexed 2, skipped 1, pruned 1, failed 0."),
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

test("covers workflow configuration edge cases", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Open workflow editor" }).click();
  await page.getByRole("button", { name: "Reset" }).click();

  await expect.poll(() => mocks.workflowResets.length).toBe(1);
  await expect(page.getByText("No workflow diagnostics.")).toBeVisible();
});

test("renders workflow configuration save failures", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open workflow editor" }).click();

  await page.unroute("**/api/workflows");
  await page.route("**/api/workflows", async (route) => {
    if (route.request().method() === "PUT") {
      await route.fulfill({
        json: { detail: "workflow save failed" },
        status: 500,
      });
      return;
    }

    await route.fallback();
  });
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("workflow save failed")).toBeVisible();
  await expect(page.getByText("Saved workflows.")).toHaveCount(0);
});

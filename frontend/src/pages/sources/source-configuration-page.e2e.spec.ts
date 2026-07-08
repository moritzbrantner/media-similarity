import { expect, test } from "@playwright/test";

import { mockEndpointFailure } from "../../../../tests/e2e/support/api-mocks";
import {
  makeSourceConfigResponse,
  modelsResponse,
  sourceConfigResponse,
} from "../../../../tests/e2e/support/media-fixtures";
import { installUiTestMocks, resetApiMocks } from "../../../../tests/e2e/support/page-objects";

test.beforeEach(async ({ page }) => {
  await installUiTestMocks(page);
});

test("edits, previews, and saves media sources", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/sources");

  await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
  await page.getByRole("button", { name: "Add Source" }).click();
  await page.getByLabel("Source spec").last().fill("/new-media");

  await page.getByRole("button", { name: "Preview" }).click();
  await expect.poll(() => mocks.sourceConfigPreviews.length).toBe(1);
  await expect(page.getByText("2 item(s)").first()).toBeVisible();
  await expect(page.getByText("Feature degraded: visual_embedding").first()).toBeVisible();

  await page.getByRole("button", { name: "Save" }).click();
  await expect.poll(() => mocks.sourceConfigPuts.length).toBe(1);
  await expect(mocks.sourceConfigPuts[0]).toMatchObject({
    sources: ["/images", "/archive", "/new-media"],
  });
  await expect(page.getByText("Saved source configuration.")).toBeVisible();
});

test("calls model actions from the source configuration panel", async ({ page }) => {
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
        { ...modelsResponse.models[1], cached: true },
      ],
    },
  });
  await page.goto("/sources");

  await expect(page.getByText("blocking")).toBeVisible();
  await page.getByRole("button", { exact: true, name: "Download" }).first().click();
  await expect
    .poll(() => mocks.modelDownloads)
    .toEqual([
      {
        model: "xenova-clip-vit-base-patch32-onnx",
        role: "visual_embedding",
      },
    ]);

  await page.getByRole("button", { exact: true, name: "Enable" }).last().click();
  await expect
    .poll(() => mocks.modelEnables)
    .toEqual([{ model: "base.en", role: "audio_transcription" }]);
});

test("disables source saves when the source file is read-only", async ({ page }) => {
  await resetApiMocks(page, {
    sourceConfig: makeSourceConfigResponse({
      media_sources_file: "/app/data/media-sources.txt",
      media_sources_seed_file: "/app/config/media-sources.txt",
      media_sources_writable: false,
    }),
  });
  await page.goto("/sources");

  await expect(page.getByText("Stored in /app/data/media-sources.txt")).toBeVisible();
  await expect(page.getByText("Seeded from /app/config/media-sources.txt")).toBeVisible();
  await expect(page.getByText("Source configuration file is not writable.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();
});

test("renders source save and preview failures", async ({ page }) => {
  await resetApiMocks(page, {
    sourceConfig: makeSourceConfigResponse({
      sources: [
        {
          detail: "Folder does not exist",
          kind: "local",
          spec: "/missing",
          status: "unavailable",
        },
      ],
    }),
  });
  await page.goto("/sources");

  await expect(page.getByText("unavailable")).toBeVisible();
  await expect(page.getByText("Folder does not exist")).toBeVisible();

  await mockEndpointFailure(page, "**/api/source-config/preview", 500, "preview failed");
  await page.getByRole("button", { name: "Preview" }).click();
  await expect(page.getByText("preview failed")).toBeVisible();

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

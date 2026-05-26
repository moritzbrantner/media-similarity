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

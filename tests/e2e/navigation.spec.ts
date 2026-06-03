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

  await page.getByRole("button", { name: "Open workflow editor" }).click();
  await expect(page.getByRole("heading", { name: "Processing Workflows" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open workflow editor" })).toHaveAttribute(
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

test.describe("mobile viewport", () => {
  test.use({ viewport: { height: 844, width: 390 } });

  test("supports core navigation and search on mobile", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Open media configuration" }).click();
    await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save" })).toBeVisible();

    await page.getByRole("button", { name: "Open workflow editor" }).click();
    await expect(page.getByRole("heading", { name: "Processing Workflows" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save" })).toBeVisible();

    await page.getByRole("button", { name: "Open query page" }).click();
    await page.locator("#query-image").setInputFiles(imageUpload);
    await expect(page.getByRole("button", { name: "Search" })).toBeVisible();
    await page.getByRole("button", { name: "Search" }).click();

    await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  });
});

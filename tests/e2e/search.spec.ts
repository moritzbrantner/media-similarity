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

test("uploads a face query and renders people plus media matches", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("1 people, 1 media match(es)")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Ada" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
});

test("renders face search model failures", async ({ page }) => {
  await mockEndpointFailure(
    page,
    "**/api/search/face?**",
    503,
    "Face detection model is not active",
  );
  await page.goto("/");

  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("Face detection model is not active")).toBeVisible();
});

test("omits empty numeric search filters from search requests", async ({ page }) => {
  let searchUrl: string | null = null;
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    searchUrl = route.request().url();
    await route.fulfill({ json: fixtureSearchResponse });
  });
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect.poll(() => searchUrl).not.toBeNull();
  const params = new URL(searchUrl ?? "").searchParams;
  expect(params.get("min_width")).toBeNull();
  expect(params.get("max_width")).toBeNull();
  expect(params.get("min_height")).toBeNull();
  expect(params.get("max_height")).toBeNull();
  expect(params.get("min_size_bytes")).toBeNull();
  expect(params.get("max_size_bytes")).toBeNull();
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

test("updates tags on an indexed media result", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");
  await uploadAndSearch(page);

  const card = resultCard(page, "sunrise.jpg");
  await card.getByRole("textbox", { name: "Tags for sunrise.jpg" }).fill("travel, favorite");
  await card.getByRole("button", { name: "Save tags for sunrise.jpg" }).click();

  await expect
    .poll(() => mocks.mediaTagUpdates)
    .toEqual([{ id: "local-sunrise", tags: ["travel", "favorite"] }]);
  await expect(card.getByText("favorite", { exact: true })).toBeVisible();

  await card.getByRole("button", { name: "Remove tag favorite" }).click();
  await card.getByRole("textbox", { name: "Tags for sunrise.jpg" }).fill("travel, archive");
  await card.getByRole("button", { name: "Save tags for sunrise.jpg" }).click();

  await expect
    .poll(() => mocks.mediaTagUpdates)
    .toEqual([
      { id: "local-sunrise", tags: ["travel", "favorite"] },
      { id: "local-sunrise", tags: ["travel", "archive"] },
    ]);
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

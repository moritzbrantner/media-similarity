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
  await expect(page.getByLabel("Sort")).toHaveValue("relevance");
  await expectResultOrder(page, ["clip.mp4"]);
});

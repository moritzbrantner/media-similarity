import { expect, test } from "@playwright/test";

import {
  captureSearchRequests,
  mockEndpointFailure,
} from "../../../../tests/e2e/support/api-mocks";
import { imageUpload, searchResponse } from "../../../../tests/e2e/support/media-fixtures";
import { installUiTestMocks, uploadAndSearch } from "../../../../tests/e2e/support/page-objects";

test.beforeEach(async ({ page }) => {
  await installUiTestMocks(page);
});

test("uploads query media and renders search results", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await expect(page.getByRole("button", { name: "Search" })).toBeEnabled();
  await expect(page.getByText("Metadata filters")).toBeVisible();

  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("2 of 2 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
  await expect(page.getByText("Near duplicate", { exact: true })).toBeVisible();
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

test("keeps search disabled until media is selected and clears selected media", async ({
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

test("renders search API failures in-page", async ({ page }) => {
  await mockEndpointFailure(page, "**/api/search?**", 500, "search failed");
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("search failed")).toBeVisible();
});

test("sends important search filter parameters", async ({ page }) => {
  const requests = await captureSearchRequests(page, searchResponse);
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();
  await expect.poll(() => requests.count).toBe(1);

  await page.getByLabel("Name or path").fill("sunrise");
  await page.getByLabel("Person ID").fill("person-ada");
  await page.getByLabel("Media type").selectOption("static_image");
  await page.getByLabel("Source type").selectOption("local");
  await page.getByLabel("Duplicate status").selectOption("only");
  await page.getByLabel("GPS metadata").selectOption("yes");
  await page.getByLabel("Text query").fill("invoice");
  await page.getByRole("button", { name: "Search" }).click();

  await expect.poll(() => requests.count).toBe(2);
  expect(requests.requests[1]).toMatchObject({
    hasGps: "yes",
    mediaKind: "static_image",
    nearDuplicate: "only",
    ocrText: "invoice",
    personId: "person-ada",
    sourceType: "local",
  });
});

test("can save tags from search results", async ({ page }) => {
  await page.goto("/");
  await uploadAndSearch(page);

  const card = page
    .locator("article")
    .filter({ has: page.getByRole("heading", { name: "sunrise.jpg" }) });
  await card.getByRole("textbox", { name: "Tags for sunrise.jpg" }).fill("travel, favorite");
  await card.getByRole("button", { name: "Save tags for sunrise.jpg" }).click();

  await expect(card.getByText("favorite", { exact: true })).toBeVisible();
});

import { expect, test } from "@playwright/test";

import { installUiTestMocks, resetApiMocks, uploadAndSearch } from "./support/page-objects";

test.beforeEach(async ({ page }) => {
  await installUiTestMocks(page);
});

test("renders smart albums and duplicate group results", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open smart albums" }).click();

  await expect(page.getByText("Duplicate Sunrises")).toBeVisible();
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await expect(page.getByText("Duplicate group", { exact: true })).toBeVisible();
  await expect(page.getByText("2 media")).toBeVisible();
});

test("creates and previews a smart album", async ({ page }) => {
  const mocks = await resetApiMocks(page, { smartAlbums: [] });

  await page.goto("/");
  await page.getByRole("button", { name: "Open smart albums" }).click();
  await page.getByRole("button", { name: "New" }).click();
  await page.getByRole("textbox", { exact: true, name: "Name" }).fill("Duplicate review");
  await page.getByLabel("Duplicate status").selectOption("only");
  await page.getByRole("button", { name: "Preview" }).click();

  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await page.getByRole("button", { exact: true, name: "Save" }).click();

  await expect.poll(() => mocks.smartAlbumCreates.length).toBe(1);
  expect(mocks.smartAlbumCreates[0]).toMatchObject({
    criteria: { duplicate_status: "only" },
    name: "Duplicate review",
  });
});

test("seeds an album draft from current search filters", async ({ page }) => {
  const mocks = await resetApiMocks(page, { smartAlbums: [] });

  await page.goto("/");
  await uploadAndSearch(page);
  await page.getByLabel("Name or path").fill("sunrise");
  await page.getByLabel("Text query").fill("invoice");
  await page.getByRole("button", { name: "Save as album" }).click();

  await expect(page.getByRole("button", { name: "Open smart albums" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(page.getByLabel("Name or path")).toHaveValue("sunrise");
  await expect(page.getByLabel("Text in media")).toHaveValue("invoice");
  await page.getByRole("button", { exact: true, name: "Save" }).click();
  await expect.poll(() => mocks.smartAlbumCreates.length).toBe(1);
});

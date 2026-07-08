import { expect, test } from "@playwright/test";
import { installUiTestMocks } from "../../../tests/e2e/support/page-objects";

test.beforeEach(async ({ page }) => {
  await installUiTestMocks(page);
});

test("loads sources page directly and keeps navbar state in the URL", async ({ page }) => {
  await page.goto("/sources");

  await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Open media configuration" })).toHaveAttribute(
    "aria-current",
    "page",
  );

  await page.getByRole("link", { name: "Open workflow editor" }).click();
  await expect(page).toHaveURL(/\/workflows$/);
  await expect(page.getByRole("heading", { name: "Processing Workflows" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Open workflow editor" })).toHaveAttribute(
    "aria-current",
    "page",
  );
});

test.describe("mobile navbar", () => {
  test.use({ viewport: { height: 844, width: 390 } });

  test("keeps primary links and index action visible", async ({ page }) => {
    await page.goto("/sources");

    for (const name of [
      "Open query page",
      "Open smart albums",
      "Open inverse index",
      "Open media configuration",
      "Open workflow editor",
    ]) {
      await expect(page.getByRole("link", { name })).toBeVisible();
    }
    await expect(
      page.locator("header").getByRole("button", { name: "Index Sources" }),
    ).toBeVisible();
  });
});

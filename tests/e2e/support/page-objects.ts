import { expect, type Page } from "@playwright/test";

import { installDefaultApiMocks } from "./api-mocks";
import { pngPixel } from "./media-fixtures";

export async function installUiTestMocks(page: Page) {
  await installDefaultApiMocks(page);
}

export async function resetApiMocks(
  page: Page,
  options: Parameters<typeof installDefaultApiMocks>[1] = {},
) {
  for (const pattern of [
    "**/api/health",
    "**/api/index",
    "**/api/inverse-index",
    "**/api/smart-albums/preview?**",
    "**/api/smart-albums/*/results?**",
    "**/api/smart-albums/*",
    "**/api/smart-albums",
    "**/api/identities/people/*/merge",
    "**/api/identities/people/*",
    "**/api/identities/speakers/*/merge",
    "**/api/identities/speakers/*",
    "**/api/jobs/index",
    "**/api/jobs",
    "**/api/jobs/*/events",
    "**/api/jobs/*/cancel",
    "**/api/models",
    "**/api/models/*/download",
    "**/api/models/*/enable",
    "**/api/indexed-media/*/tags",
    "**/api/indexed-media/*",
    "**/api/source-config",
    "**/api/search?**",
    "**/thumbnails/**",
  ]) {
    await page.unroute(pattern).catch(() => undefined);
  }

  return installDefaultApiMocks(page, options);
}

export async function mockSearchResponse(page: Page, response: unknown) {
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    await route.fulfill({ json: response });
  });
}

export async function uploadAndSearch(page: Page, name = "query.png") {
  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name,
  });
  await page.getByRole("button", { name: "Search" }).click();
}

export async function expectResultOrder(page: Page, filenames: string[]) {
  await expect(page.locator("article h3")).toHaveText(filenames);
}

export function resultCard(page: Page, filename: string) {
  return page.locator("article").filter({ has: page.getByRole("heading", { name: filename }) });
}

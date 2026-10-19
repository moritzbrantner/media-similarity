import assert from "node:assert/strict";
import { chromium } from "@playwright/test";

// Called by the opt-in native acceptance test with its isolated service and fixtures.
const [url, queryPath, stage] = process.argv.slice(2);
assert(url && queryPath && stage, "expected service URL, query image, lifecycle stage");
assert.equal(new URL(url).hostname, "127.0.0.1");
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  await page.goto(url);
  await page.locator("#query-image").setInputFiles(queryPath);
  await page.getByLabel("Name or path", { exact: true }).fill("dated");
  const responsePromise = page.waitForResponse(
    (response) => response.url().includes("/api/search?") && response.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const response = await responsePromise;
  assert.equal(response.status(), 200);
  const result = await response.json();
  assert.equal(result.count, 1);
  assert.equal(result.results[0].image.filename, "dated-landscape.jpg");
  assert.equal(result.results[0].image.width, stage === "initial" ? 80 : 20);
  await page.getByRole("heading", { name: "dated-landscape.jpg", exact: true }).waitFor();
  await page.getByLabel("Name or path", { exact: true }).fill("portrait");
  const nextResponse = page.waitForResponse(
    (response) => response.url().includes("/api/search?") && response.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const portrait = await (await nextResponse).json();
  assert.equal(portrait.count, stage === "initial" ? 1 : 0);
  if (stage === "initial") {
    await page.getByRole("heading", { name: "portrait.jpg", exact: true }).waitFor();
  }
  console.log(
    JSON.stringify({
      stage,
      mediaId: result.results[0].image.id,
      width: result.results[0].image.width,
      portraitCount: portrait.count,
    }),
  );
} finally {
  await browser.close();
}

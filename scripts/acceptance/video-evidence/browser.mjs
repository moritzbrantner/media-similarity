import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";

import fs from "node:fs/promises";
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("response", (r) => {
  if (r.status() >= 400) console.log("HTTP", r.status(), r.url());
});
await page.goto(baseURL + "/sources");
await page.waitForTimeout(1500);
console.log(await page.locator("body").innerText());
await page
  .getByLabel("Source spec")
  .first()
  .fill(root + "/sources");
await page.getByRole("button", { name: "Save", exact: true }).click();
await page.getByText("Saved source configuration.").waitFor();
console.log("SAVED");
await page.getByRole("button", { name: "Index Sources", exact: true }).first().click();
await page.waitForTimeout(1500);
console.log("AFTER INDEX", await page.locator("body").innerText());
await page.screenshot({ path: root + "/sources.png", fullPage: true });
console.log("ERRORS", errors);
await browser.close();

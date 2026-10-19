import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";
import fs from "node:fs/promises";
import assert from "node:assert/strict";
const b = await chromium.launch();
const p = await b.newPage({ viewport: { width: 1440, height: 1100 } });
await p.goto(baseURL + "/");
await p.locator("#ocr-text-query").fill("country");
let [r] = await Promise.all([
  p.waitForResponse((r) => r.url().includes("/api/search?"), { timeout: 180000 }),
  p.getByRole("button", { name: "Search", exact: true }).click(),
]);
assert.equal(r.status(), 200);
let x = await r.json();
await fs.writeFile(root + "/degraded-browser-search.json", JSON.stringify(x, null, 2));
const vs = x.results.filter((r) => r.image.media_kind === "video_scene");
assert.equal(vs.length, 2);
assert(
  vs.every(
    (r) =>
      ["unavailable", "failed"].includes(r.image.video_evidence.ocr_status.state) &&
      r.image.video_evidence.ocr_status.reason.includes("unavailable") &&
      r.image.video_evidence.speech_status.state === "completed",
  ),
);
await p.getByText("Video evidence", { exact: true }).first().click();
let body = await p.locator("body").innerText();
console.log(body);
assert(/Video OCR: (Unavailable|Failed)/.test(body));
assert(body.includes("unavailable"));
assert(body.includes("country"));
await p.screenshot({ path: root + "/final-degraded.png", fullPage: true });
console.log(
  "PASS unmocked degraded UI keeps transcript query/results and explicit failed/unavailable OCR",
);
await b.close();

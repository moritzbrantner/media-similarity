import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";
import fs from "node:fs/promises";
import assert from "node:assert/strict";
const b = await chromium.launch();
const p = await b.newPage({ viewport: { width: 1440, height: 1100 } });
const errors = [];
p.on("pageerror", (e) => errors.push(String(e)));
async function search(name, file, text = "", face = false) {
  await p.goto(baseURL + "/");
  if (face) await p.getByRole("button", { name: "Face", exact: true }).click();
  if (file) await p.locator("#query-image").setInputFiles(file);
  if (text) await p.locator("#ocr-text-query").fill(text);
  let [r] = await Promise.all([
    p.waitForResponse((r) => r.url().includes(face ? "/api/search/face?" : "/api/search?"), {
      timeout: 180000,
    }),
    p.getByRole("button", { name: "Search", exact: true }).click(),
  ]);
  assert.equal(r.status(), 200, await r.text());
  let x = await r.json();
  await fs.writeFile(`${root}/final-browser-${name}.json`, JSON.stringify(x, null, 2));
  await p.waitForTimeout(250);
  console.log("PASS search", name, x.count ?? x.media_matches?.length);
  return x;
}
let f = await search("face", root + "/query-obama.jpg", "", true);
console.log("FACE SUMMARY", JSON.stringify(f).slice(0, 1000));
assert(
  (f.results ?? f.media_matches ?? []).some(
    (r) => (r.result?.image ?? r.image ?? r).filename === "obama.jpg",
  ),
);
await p.goto(baseURL + "/albums");
await p.getByText("Acceptance budget", { exact: true }).click();
await p.getByRole("heading", { name: "budget.png" }).waitFor();
await p.screenshot({ path: `${root}/final-album.png`, fullPage: true });
console.log("PASS saved smart album");
assert.deepEqual(errors, []);
console.log("PASS no pageerrors");
await b.close();

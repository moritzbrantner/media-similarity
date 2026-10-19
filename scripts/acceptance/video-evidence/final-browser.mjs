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
let t = await search("text", null, "BUDGET");
assert.equal(t.count, 2);
assert(t.results.every((r) => r.image.ocr_text.includes("BUDGET")));
await p.getByText("Video evidence", { exact: true }).click();
assert((await p.locator("body").innerText()).includes(root + "/sources/context.mp4"));
await p.screenshot({ path: `${root}/final-evidence.png`, fullPage: true });
assert(
  (
    await p.locator("img").evaluateAll((xs) => xs.map((x) => x.complete && x.naturalWidth > 0))
  ).every(Boolean),
);
let image = await search("image", `${root}/sources/budget.png`);
assert.equal(image.results[0].image.filename, "budget.png");
assert.equal(image.results[0].hash_distance, 0);
assert.equal(image.query_visual_embedding_degraded, false);
await p
  .locator("select")
  .filter({ has: p.locator('option[value="phash_distance"]') })
  .selectOption("phash_distance");
let ds = await p.locator("article h3").allTextContents();
console.log("SORT phash", ds);
await p
  .locator("select")
  .filter({ has: p.locator('option[value="phash_distance"]') })
  .selectOption("filename");
let names = await p.locator("article h3").allTextContents();
assert.deepEqual(
  names,
  [...names].sort((a, b) => a.localeCompare(b)),
);
console.log("PASS sort controls");
await p.getByLabel("Name or path").fill("context");
await p.locator("#limit").fill("1");
let [fr] = await Promise.all([
  p.waitForResponse((r) => r.url().includes("/api/search?")),
  p.getByRole("button", { name: "Search", exact: true }).click(),
]);
let filtered = await fr.json();
assert.equal(filtered.count, 1);
assert(filtered.results.every((r) => r.image.filename.includes("context")));
console.log("PASS filter limit");
let v = await search("video", `${root}/sources/context.mp4`);
assert.equal(v.query_media_kind, "video");
assert.equal(v.scenes.length, 2);
assert(v.results.some((r) => r.image.video_evidence?.visible_text?.length));
await p.getByRole("button", { name: /Scene 2/ }).click();
await p.waitForTimeout(250);
assert.equal(
  (await p.locator("article h3").allTextContents())[0],
  v.scenes[1].results[0].image.filename,
);
console.log("PASS source scene selection");
await p.screenshot({ path: `${root}/final-video.png`, fullPage: true });
let a = await search("audio", `${root}/sources/jfk.wav`);
assert.equal(a.query_media_kind, "audio");
assert(a.query_audio_analysis.transcript_text.toLowerCase().includes("country"));
console.log("PASS actual ASR audio query");
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

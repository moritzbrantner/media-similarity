import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";
import fs from "node:fs/promises";
import assert from "node:assert/strict";
const snap = JSON.parse(await fs.readFile(root + "/face-after-first-rebuild.json"));
const s = snap.result.points
  .map((x) => x.payload)
  .find((x) => x.filename === "context.mp4 scene 002");
const b = await chromium.launch();
const p = await b.newPage();
for (const key of ["full_video_url", "scene_clip_url"]) {
  const url = baseURL + s[key];
  const range = await p.request.get(url, { headers: { Range: "bytes=0-31" } });
  assert.equal(range.status(), 206);
  assert.equal((await range.body()).length, 32);
  await p.goto(url, { waitUntil: "domcontentloaded" });
  await p.waitForFunction(() => document.querySelector("video")?.readyState >= 1);
  const result = await p.locator("video").evaluate(async (v) => {
    v.muted = true;
    v.currentTime = 1;
    await v.play();
    await new Promise((r) => setTimeout(r, 300));
    v.pause();
    return {
      duration: v.duration,
      currentTime: v.currentTime,
      videoWidth: v.videoWidth,
      error: v.error?.message,
    };
  });
  assert(result.currentTime > 1 && result.videoWidth === 640 && !result.error);
  console.log("PASS actual browser playback/range206", key, result);
}
await b.close();

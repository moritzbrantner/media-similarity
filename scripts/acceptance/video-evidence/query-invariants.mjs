import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";
import fs from "node:fs/promises";
import assert from "node:assert/strict";

async function query(name, params, file) {
  const form = new FormData();
  if (file)
    form.append(
      "file",
      new File([await fs.readFile(file)], file.split("/").at(-1), { type: "image/png" }),
    );
  const r = await fetch(baseURL + "/api/search?" + new URLSearchParams(params), {
    method: "POST",
    body: form,
    signal: AbortSignal.timeout(120000),
  });
  const text = await r.text();
  assert(r.ok, text);
  const x = JSON.parse(text);
  await fs.writeFile(`${root}/${name}.json`, JSON.stringify(x, null, 2));
  return x;
}
let x = await query("typed-video-country", {
  ocr_text: "country",
  media_kind: "video_scene",
  limit: "12",
});
assert.equal(x.count, 2);
assert(
  x.results.every(
    (r) => r.image.media_kind === "video_scene" && r.image.video_evidence.speech.length,
  ),
);
console.log("PASS typed video phrase filter:2 contextual evidence scenes");
x = await query("typed-audio-country", { ocr_text: "country", media_kind: "audio", limit: "12" });
assert.equal(x.count, 4);
assert(x.results.every((r) => r.image.media_kind === "audio"));
console.log("PASS typed audio phrase filter:4 audio bits");
x = await query(
  "near-duplicate-only",
  { near_duplicate: "only", limit: "12" },
  `${root}/sources/budget.png`,
);
assert(x.count >= 2);
assert(x.results.every((r) => r.near_duplicate && r.hash_distance <= 8));
assert.equal(x.results[0].image.filename, "budget.png");
assert(x.results[0].vector_score > 0.999);
assert.equal(x.query_visual_embedding_degraded, false);
console.log("PASS actual CLIP exact self/cosine/pHash duplicate-only invariant");
x = await query(
  "near-duplicate-exclude",
  { near_duplicate: "exclude", limit: "12" },
  `${root}/sources/budget.png`,
);
assert(x.results.every((r) => !r.near_duplicate && r.hash_distance > 8));
console.log("PASS pHash exclusion invariant");

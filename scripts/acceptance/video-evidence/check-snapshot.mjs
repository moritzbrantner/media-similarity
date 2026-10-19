import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";
import fs from "node:fs/promises";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
const r = await fetch(qdrantURL + "/collections/media88_acceptance/points/scroll", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ limit: 100, with_payload: true, with_vector: false }),
});
assert(r.ok);
const x = await r.json();
await fs.writeFile(`${root}/final-verified-snapshot.json`, JSON.stringify(x, null, 2));
const ps = x.result.points.map((p) => p.payload).filter((p) => p.point_kind === "media");
assert.equal(ps.length, 8);
const scenes = ps
  .filter((p) => p.media_kind === "video_scene")
  .sort((a, b) => a.scene_index - b.scene_index);
assert.equal(scenes.length, 2);
const hash = createHash("sha256")
  .update(await fs.readFile(`${root}/sources/context.mp4`))
  .digest("hex");
for (const [i, s] of scenes.entries()) {
  assert.equal(s.source_item_uri, `${root}/sources/context.mp4`);
  assert.equal(s.video_evidence.content_hash, hash);
  assert.deepEqual([s.scene_start_seconds, s.scene_end_seconds], i ? [6, 11] : [0, 6]);
  const e = s.video_evidence;
  assert.equal(e.ocr_status.state, "completed");
  assert.equal(e.speech_status.state, "completed");
  assert.equal(e.visible_text.length, 1);
  assert.equal(s.ocr_frames.length, 1);
  assert.equal(e.visible_text[0].text, i ? "REVENUE 2027" : "BUDGET 2026");
  assert.equal(s.ocr_text, e.visible_text[0].text);
  const t = e.visible_text[0];
  assert.equal(t.observations.length, 4);
  assert(
    t.observations.every(
      (o) =>
        o.timestamp_seconds >= s.scene_start_seconds && o.timestamp_seconds < s.scene_end_seconds,
    ),
  );
  assert.equal(t.producer.input_hash, "sha256:" + hash);
  assert(t.producer.producer.includes("visual-analysis"));
  assert(t.producer.model_version.includes("tesseract"));
  assert(e.relations.some((r) => r.subject === e.scene_id && r.object === t.id));
  assert(e.speech.length > 0);
  for (const speech of e.speech) {
    assert(
      speech.start_seconds < s.scene_end_seconds && speech.end_seconds > s.scene_start_seconds,
    );
    assert(speech.text.toLowerCase().includes("country"));
    assert.equal(speech.producer.input_hash, "sha256:" + hash);
  }
  console.log(
    "PASS scene source provenance",
    s.filename,
    [s.scene_start_seconds, s.scene_end_seconds],
    t.observations.map((o) => o.timestamp_seconds),
    e.speech.map((t) => [t.start_seconds, t.end_seconds]),
  );
  for (const key of ["thumbnail_url", "full_video_url", "scene_clip_url"]) {
    const a = await fetch(baseURL + s[key]);
    assert.equal(a.status, 200);
    assert((await a.arrayBuffer()).byteLength > 0);
  }
}
assert.equal(scenes[0].video_evidence.speech[0].id, scenes[1].video_evidence.speech[0].id);
console.log(
  "PASS real OCR repeat dedup, source-relative speech/relations/receipts/hash, thumbnails/source/scene artifacts",
);

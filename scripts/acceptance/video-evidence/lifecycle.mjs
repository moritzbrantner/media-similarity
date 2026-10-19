import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";
import fs from "node:fs/promises";
import assert from "node:assert/strict";

const write = (name, x) => fs.writeFile(`${root}/${name}.json`, JSON.stringify(x, null, 2));
async function api(path, { data, method, q = false } = {}) {
  const r = await fetch((q ? qdrantURL : baseURL) + path, {
    method: method ?? (data === undefined ? "GET" : "POST"),
    ...(data === undefined
      ? {}
      : { body: JSON.stringify(data), headers: { "Content-Type": "application/json" } }),
    signal: AbortSignal.timeout(180000),
  });
  const text = await r.text();
  assert(r.ok, `${r.status} ${path} ${text}`);
  return JSON.parse(text);
}
async function snapshot(name, vectors = false) {
  const x = await api("/collections/media88_acceptance/points/scroll", {
    q: true,
    data: { limit: 100, with_payload: true, with_vector: vectors },
  });
  await write(name, x);
  return x.result.points;
}
const media = (points) =>
  Object.fromEntries(
    points.filter((p) => p.payload.point_kind === "media").map((p) => [p.id, p.payload]),
  );
async function index(name) {
  const queued = await api("/api/jobs/index", { data: {} });
  let j;
  for (let n = 0; n < 360; n++) {
    j = await api("/api/jobs/" + queued.spec.id);
    if (["Succeeded", "Failed", "Cancelled"].includes(j.status)) break;
    await new Promise((r) => setTimeout(r, 1000));
  }
  await write(name, j);
  assert.equal(j.status, "Succeeded", JSON.stringify(j));
  console.log(name, j.metadata);
  return j;
}
const mode = process.argv[2];
if (mode === "face-refresh") {
  const before = media(await snapshot("face-before-physical-fix"));
  const path = `${root}/sources/obama.jpg`;
  const now = new Date();
  await fs.utimes(path, now, now);
  const j = await index("face-physical-refresh");
  assert.equal(j.metadata.indexed, "1");
  assert.equal(j.metadata.already_indexed, "3");
  const points = await snapshot("face-after-physical-fix");
  const after = media(points);
  const old = Object.values(before).find((p) => p.filename === "obama.jpg"),
    current = Object.values(after).find((p) => p.filename === "obama.jpg");
  assert.deepEqual(current.faces, old.faces);
  assert.deepEqual(current.people, old.people);
  const faces = points.filter((p) => p.payload.point_kind === "face");
  assert(faces.length >= 1);
  assert(
    faces.every(
      (p) =>
        /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(p.id) &&
        p.payload.face_id.includes("#face="),
    ),
  );
  console.log(
    "PASS physical UUID persisted; logical faces/person IDs unchanged",
    faces.map((p) => ({ id: p.id, face_id: p.payload.face_id })),
  );
} else if (mode === "reindex") {
  const before = media(await snapshot("final-before-reindex"));
  const j = await index("final-reindex");
  const after = media(await snapshot("final-after-reindex"));
  assert.deepEqual(after, before);
  assert.equal(j.metadata.indexed, "0");
  assert.equal(j.metadata.already_indexed, "4");
  console.log("PASS unchanged reindex: all IDs/payloads equal and4 files current");
} else if (mode === "reload") {
  const before = media(
    JSON.parse(await fs.readFile(`${root}/face-after-physical-fix.json`)).result.points,
  );
  const after = media(await snapshot("final-after-reload"));
  assert.deepEqual(after, before);
  const j = await index("final-reload-reindex");
  assert.equal(j.metadata.indexed, "0");
  assert.equal(j.metadata.already_indexed, "4");
  console.log("PASS native reload preserves IDs/payload/evidence/current-source state");
} else if (mode === "rebuild") {
  const before = await snapshot("final-before-rebuild", true);
  await api("/collections/media88_acceptance", { q: true, method: "DELETE" });
  await index("final-rebuild");
  const after = await snapshot("final-after-rebuild", true);
  assert.deepEqual(media(after), media(before));
  const a = Object.fromEntries(after.map((p) => [p.id, p.vector]));
  assert.deepEqual(Object.keys(a).sort(), before.map((p) => p.id).sort());
  for (const p of before)
    for (const [role, vector] of Object.entries(p.vector)) {
      assert.equal(a[p.id][role].length, vector.length);
      assert(vector.every((v, i) => Math.abs(v - a[p.id][role][i]) < 1e-6));
    }
  console.log(
    "PASS collection rebuild: all media payloads/evidence/IDs and physical face IDs identical; named vectors equal within1e-6",
  );
} else if (mode === "degrade") {
  await index("final-degraded-index");
  const points = await snapshot("final-degraded");
  const videos = Object.values(media(points)).filter((p) => p.media_kind === "video_scene");
  assert.equal(videos.length, 2);
  for (const p of videos) {
    assert(["unavailable", "failed"].includes(p.video_evidence.ocr_status.state));
    assert(p.video_evidence.ocr_status.reason.includes("unavailable"));
    assert.equal(p.video_evidence.speech_status.state, "completed");
    assert.equal(p.video_evidence.visible_text.length, 0);
    assert(p.video_evidence.speech.length > 0);
    console.log(p.filename, p.video_evidence.ocr_status);
  }
  console.log(
    "PASS real missing OCR executable: explicit failed/unavailable coverage; actual speech retained",
  );
} else throw Error("Unknown mode " + mode);

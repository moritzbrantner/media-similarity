import { baseURL } from "./environment.mjs";
import assert from "node:assert/strict";
for (let i = 0; i < 600; i++) {
  const jobs = await (await fetch(baseURL + "/api/jobs")).json();
  const index = jobs
    .filter((j) => j.spec.kind === "index.manual")
    .sort((a, b) => b.created_at.localeCompare(a.created_at))[0];
  if (index && ["Succeeded", "Failed", "Cancelled"].includes(index.status)) {
    assert.equal(index.status, "Succeeded", JSON.stringify(index));
    assert.equal(index.metadata.failed, "0");
    console.log("PASS initial UI-triggered indexing", index.metadata);
    process.exit(0);
  }
  await new Promise((r) => setTimeout(r, 1000));
}
throw Error("Indexing did not finish");

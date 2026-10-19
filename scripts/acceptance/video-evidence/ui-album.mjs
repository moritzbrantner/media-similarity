import { root, repo, baseURL, qdrantURL, chromium } from "./environment.mjs";

const b = await chromium.launch();
const p = await b.newPage();
p.on("response", async (r) => {
  if (r.url().includes("smart-albums"))
    console.log(r.request().method(), r.url(), r.status(), await r.text());
});
await p.goto(baseURL + "/albums");
await p.getByRole("button", { name: "New", exact: true }).click();
await p.getByRole("textbox", { name: "Name", exact: true }).fill("Acceptance budget");
await p.getByLabel("Text in media").fill("BUDGET");
await p.getByRole("button", { name: "Preview", exact: true }).click();
await p.waitForTimeout(500);
console.log("SAVE BUTTONS", await p.getByRole("button", { name: "Save", exact: true }).count());
await p.getByRole("button", { name: "Save", exact: true }).first().click();
await p.waitForTimeout(1000);
console.log(await p.locator("body").innerText());
await p.reload();
await p.getByText("Acceptance budget", { exact: true }).waitFor();
console.log("RELOAD PERSISTED");
await p.screenshot({ path: root + "/album-real.png", fullPage: true });
await b.close();

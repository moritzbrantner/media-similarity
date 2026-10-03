import { expect, test } from "@playwright/test";

import {
  installDefaultApiMocks,
  mockEndpointFailure,
} from "../../../../tests/e2e/support/api-mocks";
import { inverseIndexResponse } from "../../../../tests/e2e/support/media-fixtures";

test("renames person and speaker identities", async ({ page }) => {
  const api = await installDefaultApiMocks(page, { inverseIndex: inverseIndexWithExtraEntries() });
  await page.goto("/registry");

  await page.getByRole("button", { name: "Rename Ada" }).click();
  await page.getByRole("textbox", { name: "Label for Ada" }).fill("Ada Lovelace");
  await page.getByRole("button", { name: "Save label for Ada" }).click();

  await expect(page.getByRole("heading", { name: "Ada Lovelace" })).toBeVisible();

  await page.getByRole("button", { name: "Rename Voice 1" }).click();
  await page.getByRole("textbox", { name: "Label for Voice 1" }).fill("Alice");
  await page.getByRole("button", { name: "Save label for Voice 1" }).click();

  await expect(page.getByRole("heading", { name: "Alice" })).toBeVisible();
  expect(api.identityRenames).toEqual([
    { id: "person-0001", kind: "person", label: "Ada Lovelace" },
    { id: "voice-0001", kind: "speaker", label: "Alice" },
  ]);
});

test("merges identities with confirmation", async ({ page }) => {
  const api = await installDefaultApiMocks(page, { inverseIndex: inverseIndexWithExtraEntries() });
  await page.goto("/registry");

  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Merge selected people into Ada?");
    await dialog.accept();
  });
  await page.getByRole("button", { name: "Merge into Ada" }).click();
  await page.getByRole("checkbox", { name: /Grace/ }).check();
  await page.getByRole("button", { name: "Confirm" }).click();

  await expect(page.getByText("person-0002")).toHaveCount(0);
  expect(api.identityMerges).toEqual([
    { kind: "person", sourceIds: ["person-0002"], targetId: "person-0001" },
  ]);
});

test("renders identity mutation failures without changing local state", async ({ page }) => {
  const api = await installDefaultApiMocks(page, { inverseIndex: inverseIndexWithExtraEntries() });
  await mockEndpointFailure(page, "**/api/identities/people/*", 500, "rename failed");
  await page.goto("/registry");

  await page.getByRole("button", { name: "Rename Ada" }).click();
  await page.getByRole("textbox", { name: "Label for Ada" }).fill("Ada Failed");
  await page.getByRole("button", { name: "Save label for Ada" }).click();

  await expect(page.getByText("rename failed")).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Label for Ada" })).toHaveValue("Ada Failed");
  await expect(page.getByText("person-0001")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Ada Failed" })).toHaveCount(0);
  expect(api.identityRenames).toHaveLength(0);
});

function inverseIndexWithExtraEntries() {
  const response = JSON.parse(JSON.stringify(inverseIndexResponse)) as typeof inverseIndexResponse;
  response.people.push({
    ...response.people[0],
    face_count: 1,
    id: "person-0002",
    label: "Grace",
    locations: [
      {
        ...response.people[0].locations[0],
        filename: "grace.png",
        media_id: "import-grace",
        path: "/archive/portraits/grace.png",
        relative_path: "portraits/grace.png",
      },
    ],
    media_count: 1,
  });
  response.speakers.push({
    ...response.speakers[0],
    id: "voice-0002",
    label: "Voice 2",
    locations: [
      {
        ...response.speakers[0].locations[0],
        filename: "panel.mp3",
        media_id: "audio-panel",
        path: "/audio/panel.mp3",
        relative_path: "panel.mp3",
      },
    ],
    segment_count: 1,
    total_seconds: 3,
  });
  return response;
}

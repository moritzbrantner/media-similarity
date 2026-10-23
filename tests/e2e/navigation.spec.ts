import { expect, test } from "@playwright/test";

import {
  captureSearchRequests,
  mockEndpointFailure,
  mockSearchResponse as mockSearchResponseRoute,
} from "./support/api-mocks";
import {
  audioUpload,
  gifUpload,
  historyStorageKey,
  imageUpload,
  makeJob,
  makeJobEvents,
  makeResult,
  makeScene,
  makeSearchResponse,
  makeSourceConfigResponse,
  pngPixel,
  pdfSearchResponse,
  pdfUpload,
  searchResponse as fixtureSearchResponse,
  sortableSearchResponse,
  sourceConfigResponse,
  videoUpload,
} from "./support/media-fixtures";
import {
  expectResultOrder,
  installUiTestMocks,
  mockSearchResponse,
  resetApiMocks,
  resultCard,
  uploadAndSearch,
} from "./support/page-objects";

test.beforeEach(async ({ page }) => {
  await installUiTestMocks(page);
});

test("renders service health and empty UI state", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Image Similarity Service" })).toBeVisible();
  await expect(page.getByText("OK")).toBeVisible();
  await expect(page.getByText("Sources: /images, /archive")).toBeVisible();
  await expect(page.getByText("No query media selected")).toBeVisible();
  await expect(page.getByText("Metadata filters")).toBeHidden();
  await expect(
    page.getByText("Choose a query image, video, audio, or PDF and run a search."),
  ).toBeVisible();
});

test("primary route graph supports links, browser history, reload and deep links", async ({
  page,
}) => {
  // Acceptance oracle stays independent of the application's route registry.
  const states = [
    { path: "/", link: "Open query page", heading: "Results" },
    { path: "/albums", link: "Open smart albums", heading: "Albums" },
    { path: "/registry", link: "Open inverse index", heading: "Inverse Index" },
    { path: "/sources", link: "Open media configuration", heading: "Media Sources" },
    { path: "/workflows", link: "Open workflow editor", heading: "Processing Workflows" },
  ] as const;

  const expectState = async (state: (typeof states)[number]) => {
    await expect.poll(() => new URL(page.url()).pathname).toBe(state.path);
    await expect(page.getByRole("heading", { name: state.heading, exact: true })).toBeVisible();
    const nav = page.getByRole("navigation", { name: "Primary" });
    for (const destination of states) {
      const link = nav.getByRole("link", { name: destination.link });
      await expect(link).toHaveAttribute("href", destination.path);
      if (destination.path === state.path) {
        await expect(link).toHaveAttribute("aria-current", "page");
      } else {
        await expect(link).not.toHaveAttribute("aria-current", "page");
      }
    }
  };

  await page.goto("/");
  await expectState(states[0]);

  for (const state of states.slice(1)) {
    await page.getByRole("link", { name: state.link }).click();
    await expectState(state);
  }
  for (const state of states.slice(0, -1).reverse()) {
    await page.goBack();
    await expectState(state);
  }
  for (const state of states.slice(1)) {
    await page.goForward();
    await expectState(state);
  }

  await page.reload();
  await expectState(states[4]);
  for (const state of states) {
    await page.goto(state.path);
    await expectState(state);
  }

  // The catch-all transition must not strand the user on an empty screen.
  await page.goto("/does-not-exist");
  await expectState(states[0]);
});

test("renders inverse index people and speaker registries", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("link", { name: "Open inverse index" }).click();

  await expect(page.getByRole("heading", { name: "Inverse Index" })).toBeVisible();
  await expect(page.getByText("Indexed media", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Depicted People" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Recognized Speakers" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Ada" })).toBeVisible();
  await expect(page.getByText("person-0001")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Voice 1" })).toBeVisible();
  await expect(page.getByText("voice-0001")).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "interview.mp3" })).toBeVisible();
  await expect(page.getByText("1.0s-8.0s")).toBeVisible();
});

test.describe("mobile viewport", () => {
  test.use({ viewport: { height: 844, width: 390 } });

  test("supports core navigation and search on mobile", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("link", { name: "Open media configuration" }).click();
    await expect(page.getByRole("heading", { name: "Media Sources" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save" })).toBeVisible();

    await page.getByRole("link", { name: "Open workflow editor" }).click();
    await expect(page.getByRole("heading", { name: "Processing Workflows" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save" })).toBeVisible();

    await page.getByRole("link", { name: "Open query page" }).click();
    await page.locator("#query-image").setInputFiles(imageUpload);
    await expect(page.getByRole("button", { name: "Search" })).toBeVisible();
    await page.getByRole("button", { name: "Search" }).click();

    await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  });
});

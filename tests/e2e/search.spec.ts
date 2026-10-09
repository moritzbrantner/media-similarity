import { expect, test } from "@playwright/test";

import {
  captureSearchRequests,
  mockEndpointFailure,
  mockSearchResponse as mockSearchResponseRoute,
} from "./support/api-mocks";
import {
  audioUpload,
  faceSearchResponse,
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

test("handles service and search API failures", async ({ page }) => {
  await mockEndpointFailure(page, "**/api/health", 503, "service unavailable");
  await mockEndpointFailure(page, "**/api/search?**", 500, "search failed");
  await page.goto("/");

  await expect(page.getByText("Sources: Service is not responding")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Image Similarity Service" })).toBeVisible();

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("search failed")).toBeVisible();
});

test("uploads query media and renders search results", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await expect(page.getByRole("button", { name: "Search" })).toBeEnabled();
  await expect(page.getByText("Metadata filters")).toBeVisible();

  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("2 of 2 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
  await expect(page.getByText("Near duplicate", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("complementary").getByRole("button", { name: /query\.png/ }),
  ).toBeVisible();
});

test("uploads a face query and renders people plus media matches", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("1 people, 1 media match(es)")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Ada" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
});

test("renders face search model failures", async ({ page }) => {
  await mockEndpointFailure(
    page,
    "**/api/search/face?**",
    503,
    "Face detection model is not active",
  );
  await page.goto("/");

  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("Face detection model is not active")).toBeVisible();
});

test("omits empty numeric search filters from search requests", async ({ page }) => {
  let searchUrl: string | null = null;
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    searchUrl = route.request().url();
    await route.fulfill({ json: fixtureSearchResponse });
  });
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect.poll(() => searchUrl).not.toBeNull();
  const params = new URL(searchUrl ?? "").searchParams;
  expect(params.get("min_width")).toBeNull();
  expect(params.get("max_width")).toBeNull();
  expect(params.get("min_height")).toBeNull();
  expect(params.get("max_height")).toBeNull();
  expect(params.get("min_size_bytes")).toBeNull();
  expect(params.get("max_size_bytes")).toBeNull();
});

test("deletes a search result from the index", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");
  await uploadAndSearch(page);

  page.on("dialog", (dialog) => dialog.accept());
  await resultCard(page, "sunrise.jpg")
    .getByRole("button", { name: /Delete sunrise\.jpg/ })
    .click();

  await expect.poll(() => mocks.deletedMediaIds).toEqual(["local-sunrise"]);
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toHaveCount(0);
});

test("updates tags on an indexed media result", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  await page.goto("/");
  await uploadAndSearch(page);

  const card = resultCard(page, "sunrise.jpg");
  await card.getByRole("textbox", { name: "Tags for sunrise.jpg" }).fill("travel, favorite");
  await card.getByRole("button", { name: "Save tags for sunrise.jpg" }).click();

  await expect
    .poll(() => mocks.mediaTagUpdates)
    .toEqual([{ id: "local-sunrise", tags: ["travel", "favorite"] }]);
  await expect(card.getByText("favorite", { exact: true })).toBeVisible();

  await card.getByRole("button", { name: "Remove tag favorite" }).click();
  await card.getByRole("textbox", { name: "Tags for sunrise.jpg" }).fill("travel, archive");
  await card.getByRole("button", { name: "Save tags for sunrise.jpg" }).click();

  await expect
    .poll(() => mocks.mediaTagUpdates)
    .toEqual([
      { id: "local-sunrise", tags: ["travel", "favorite"] },
      { id: "local-sunrise", tags: ["travel", "archive"] },
    ]);
});

test("keeps search disabled until media is selected and clears the selected media", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Search" })).toBeDisabled();

  await page.locator("#query-image").setInputFiles(imageUpload);
  await expect(page.getByRole("button", { name: "Search" })).toBeEnabled();
  await expect(page.getByText("Metadata filters")).toBeVisible();

  await page.getByRole("button", { name: "Clear selected media" }).click();

  await expect(page.getByRole("button", { name: "Search" })).toBeDisabled();
  await expect(page.getByText("Metadata filters")).toBeHidden();
  await expect(page.getByText("No query media selected")).toBeVisible();
});

test("handles pending search, empty results, and search errors", async ({ page }) => {
  let resumeSearch: (() => void) | null = null;
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    await new Promise<void>((resolve) => {
      resumeSearch = resolve;
    });
    await route.fulfill({
      json: makeSearchResponse({
        count: 0,
        results: [],
      }),
    });
  });
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("Searching indexed media.").first()).toBeVisible();
  await expect(page.getByLabel("Loading search results")).toBeVisible();

  resumeSearch?.();

  await expect(page.getByText("0 of 0 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByText("No indexed media matched this query.")).toBeVisible();

  await mockEndpointFailure(page, "**/api/search?**", 500, "search failed after retry");
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("search failed after retry")).toBeVisible();
});

test("handles all query media preview types", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await expect(page.getByAltText("Query preview")).toBeVisible();

  await page.getByRole("button", { name: "Clear selected media" }).click();
  await page.locator("#query-image").setInputFiles(gifUpload);
  await expect(page.getByAltText("Query preview")).toBeVisible();
  await page.getByRole("button", { name: "Search" }).click();
  await expect(
    page.getByRole("complementary").getByRole("button", { name: /query\.gif/ }),
  ).toBeVisible();

  await page.locator("#query-image").setInputFiles(videoUpload);
  await expect(page.locator("video[controls]")).toBeVisible();

  await page.locator("#query-image").setInputFiles(audioUpload);
  await expect(page.locator("audio[controls]")).toBeVisible();

  await page.locator("#query-image").setInputFiles(pdfUpload);
  await expect(page.getByText("PDF query selected")).toBeVisible();
});

test("keeps face matches current after tag edits and deletion", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  let faceSearches = 0;
  await page.route("**/api/search/face?**", async (route) => {
    faceSearches += 1;
    await route.fulfill({
      json:
        mocks.deletedMediaIds.length === 0
          ? faceSearchResponse
          : { ...faceSearchResponse, people: [], results: [] },
    });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();
  const card = resultCard(page, "portrait.png");
  await expect(card).toBeVisible();
  await card.getByRole("textbox", { name: "Tags for portrait.png" }).fill("identity-match");
  await card.getByRole("button", { name: "Save tags for portrait.png" }).click();
  await expect(card.getByText("identity-match", { exact: true })).toBeVisible();
  page.on("dialog", (dialog) => dialog.accept());
  await card.getByRole("button", { name: /Delete portrait\.png/ }).click();
  await expect.poll(() => mocks.deletedMediaIds).toEqual(["import-portrait"]);
  await expect.poll(() => faceSearches).toBe(2);
  await expect(page.getByRole("heading", { name: "portrait.png" })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Ada" })).toHaveCount(0);
  await expect(page.getByText("0 people, 0 media match(es)")).toBeVisible();
});

test("preserves surviving face matches when the post-delete refresh fails", async ({ page }) => {
  const mocks = await resetApiMocks(page);
  let faceSearches = 0;
  const survivingResults = faceSearchResponse.results.map((match) => ({
    ...match,
    result: {
      ...match.result,
      image: { ...match.result.image, id: "survivor", filename: "survivor.png" },
    },
  }));
  await page.route("**/api/search/face?**", async (route) => {
    faceSearches += 1;
    if (faceSearches > 1) {
      await route.fulfill({ status: 503, json: { error: "refresh unavailable" } });
      return;
    }
    await route.fulfill({
      json: {
        ...faceSearchResponse,
        results: [...faceSearchResponse.results, ...survivingResults],
      },
    });
  });
  await page.route("**/api/indexed-media/survivor", (route) =>
    route.fulfill({
      json: {
        deleted_points: 0,
        deleted_faces: 0,
        deleted_artifacts: 0,
        errors: ["Current deletion failure"],
      },
    }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();
  await expect(resultCard(page, "survivor.png")).toBeVisible();
  page.on("dialog", (dialog) => dialog.accept());
  await resultCard(page, "portrait.png")
    .getByRole("button", { name: /Delete portrait\.png/ })
    .click();
  await expect.poll(() => mocks.deletedMediaIds).toEqual(["import-portrait"]);
  await expect.poll(() => faceSearches).toBe(2);
  await expect(page.getByText("refresh unavailable")).toBeVisible();
  await expect(resultCard(page, "survivor.png")).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toHaveCount(0);
  await resultCard(page, "survivor.png")
    .getByRole("button", { name: /Delete survivor\.png/ })
    .click();
  await expect(page.getByText("Current deletion failure")).toBeVisible();
  await expect(page.getByText("refresh unavailable")).toHaveCount(0);
  await expect(resultCard(page, "survivor.png")).toBeVisible();
});

test("keeps both face-result edits when tag saves finish together", async ({ page }) => {
  await resetApiMocks(page);
  const matches = [
    ...faceSearchResponse.results,
    ...faceSearchResponse.results.map((match) => ({
      ...match,
      result: {
        ...match.result,
        image: { ...match.result.image, id: "second", filename: "second.png" },
      },
    })),
  ];
  await page.route("**/api/search/face?**", (route) =>
    route.fulfill({ json: { ...faceSearchResponse, results: matches } }),
  );
  let received = 0;
  let release = () => {};
  const bothRequests = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/indexed-media/*/tags", async (route) => {
    const id = new URL(route.request().url()).pathname.split("/").at(-2);
    const image = matches.find((match) => match.result.image.id === id)?.result.image;
    if (!image) {
      throw new Error("Unexpected tag target");
    }
    const body: unknown = route.request().postDataJSON();
    const tags = typeof body === "object" && body !== null && "tags" in body ? body.tags : [];
    received += 1;
    if (received === 2) {
      release();
    }
    await bothRequests;
    await route.fulfill({ json: { ...image, tags } });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();
  const first = resultCard(page, "portrait.png");
  const second = resultCard(page, "second.png");
  await first.getByRole("textbox", { name: "Tags for portrait.png" }).fill("first-edited");
  await second.getByRole("textbox", { name: "Tags for second.png" }).fill("second-edited");
  await first.getByRole("button", { name: "Save tags for portrait.png" }).click();
  await second.getByRole("button", { name: "Save tags for second.png" }).click();
  await expect(first.getByText("first-edited", { exact: true })).toBeVisible();
  await expect(second.getByText("second-edited", { exact: true })).toBeVisible();
});

test("keeps face results when deletion reports storage errors", async ({ page }) => {
  await resetApiMocks(page);
  let faceSearches = 0;
  await page.route("**/api/search/face?**", async (route) => {
    faceSearches += 1;
    await route.fulfill({ json: faceSearchResponse });
  });
  await page.route("**/api/indexed-media/*", (route) =>
    route.fulfill({
      json: {
        deleted_points: 0,
        deleted_faces: 0,
        deleted_artifacts: 0,
        errors: ["Qdrant unavailable"],
      },
    }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();
  const card = resultCard(page, "portrait.png");
  await expect(card).toBeVisible();
  page.on("dialog", (dialog) => dialog.accept());
  await card.getByRole("button", { name: /Delete portrait\.png/ }).click();
  await expect(page.getByText("Qdrant unavailable")).toBeVisible();
  await expect(card).toBeVisible();
  expect(faceSearches).toBe(1);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect.poll(() => faceSearches).toBe(2);
  await expect(page.getByText("Qdrant unavailable")).toHaveCount(0);
});

test("refreshes again when a tag write finishes during a deletion refresh", async ({ page }) => {
  await resetApiMocks(page);
  const matches = [
    ...faceSearchResponse.results,
    ...faceSearchResponse.results.map((match) => ({
      ...match,
      result: {
        ...match.result,
        image: { ...match.result.image, id: "second", filename: "second.png" },
      },
    })),
  ];
  let searches = 0;
  let releaseTag = () => {};
  const tagGate = new Promise<void>((resolve) => {
    releaseTag = resolve;
  });
  let refreshStarted = () => {};
  const refreshGate = new Promise<void>((resolve) => {
    refreshStarted = resolve;
  });
  let releaseStale = () => {};
  const staleGate = new Promise<void>((resolve) => {
    releaseStale = resolve;
  });
  await page.route("**/api/search/face?**", async (route) => {
    searches += 1;
    const request = searches;
    if (request === 2) {
      refreshStarted();
      await staleGate;
    }
    await route.fulfill({
      json: {
        ...faceSearchResponse,
        results:
          request === 1
            ? matches
            : matches
                .filter((match) => match.result.image.id === "second")
                .map((match) => ({
                  ...match,
                  result: {
                    ...match.result,
                    image: { ...match.result.image, tags: request === 2 ? [] : ["latest-tag"] },
                  },
                })),
      },
    });
  });
  await page.route("**/api/indexed-media/second/tags", async (route) => {
    await tagGate;
    await route.fulfill({
      json: {
        ...matches.find((match) => match.result.image.id === "second")?.result.image,
        tags: ["latest-tag"],
      },
    });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();
  const second = resultCard(page, "second.png");
  await second.getByRole("textbox", { name: "Tags for second.png" }).fill("latest-tag");
  await second.getByRole("button", { name: "Save tags for second.png" }).click();
  page.on("dialog", (dialog) => dialog.accept());
  await resultCard(page, "portrait.png")
    .getByRole("button", { name: /Delete portrait\.png/ })
    .click();
  await refreshGate;
  releaseTag();
  await expect.poll(() => searches).toBe(3);
  await expect(second.getByText("latest-tag", { exact: true })).toBeVisible();
  releaseStale();
  await expect(second.getByText("latest-tag", { exact: true })).toBeVisible();
});

test("prunes successfully deleted face media while surfacing cleanup errors", async ({ page }) => {
  await resetApiMocks(page);
  let searches = 0;
  await page.route("**/api/search/face?**", async (route) => {
    searches += 1;
    await route.fulfill({
      json:
        searches === 1 ? faceSearchResponse : { ...faceSearchResponse, people: [], results: [] },
    });
  });
  await page.route("**/api/indexed-media/*", (route) =>
    route.fulfill({
      json: {
        deleted_points: 1,
        deleted_faces: 0,
        deleted_artifacts: 0,
        errors: ["Artifact cleanup failed"],
      },
    }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "Face" }).click();
  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByRole("button", { name: "Search" }).click();
  const card = resultCard(page, "portrait.png");
  await expect(card).toBeVisible();
  page.on("dialog", (dialog) => dialog.accept());
  await card.getByRole("button", { name: /Delete portrait\.png/ }).click();
  await expect.poll(() => searches).toBe(2);
  await expect(page.getByRole("heading", { name: "portrait.png" })).toHaveCount(0);
  await expect(page.getByText("Artifact cleanup failed")).toBeVisible();
});

for (const mutation of ["delete", "tag"] as const) {
  test(`does not restore an old face query after a pending ${mutation}`, async ({ page }) => {
    await resetApiMocks(page);
    let searches = 0;
    await page.route("**/api/search/face?**", async (route) => {
      searches += 1;
      await route.fulfill({ json: faceSearchResponse });
    });
    let release = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let requestStarted = () => {};
    const started = new Promise<void>((resolve) => {
      requestStarted = resolve;
    });
    const endpoint =
      mutation === "delete"
        ? "**/api/indexed-media/import-portrait"
        : "**/api/indexed-media/import-portrait/tags";
    await page.route(endpoint, async (route) => {
      requestStarted();
      await gate;
      await route.fulfill({
        json:
          mutation === "delete"
            ? { deleted_points: 1, deleted_faces: 1, deleted_artifacts: 1, errors: [] }
            : {
                ...faceSearchResponse.results.find(
                  (match) => match.result.image.id === "import-portrait",
                )?.result.image,
                tags: ["old-query-edit"],
              },
      });
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Face" }).click();
    await page.locator("#query-image").setInputFiles(imageUpload);
    await page.getByRole("button", { name: "Search" }).click();
    const card = resultCard(page, "portrait.png");
    await expect(card).toBeVisible();
    if (mutation === "delete") {
      page.on("dialog", (dialog) => dialog.accept());
      await card.getByRole("button", { name: /Delete portrait\.png/ }).click();
    } else {
      await card.getByRole("textbox", { name: "Tags for portrait.png" }).fill("old-query-edit");
      await card.getByRole("button", { name: "Save tags for portrait.png" }).click();
    }
    await started;
    await page.locator("#query-image").setInputFiles(gifUpload);
    const completed = page.waitForResponse((response) =>
      response
        .url()
        .includes(`/api/indexed-media/import-portrait${mutation === "tag" ? "/tags" : ""}`),
    );
    release();
    await completed;
    await page.evaluate(
      () =>
        new Promise<void>((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        ),
    );
    await expect(page.getByRole("heading", { name: "portrait.png" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Search", exact: true })).toBeEnabled();
    expect(searches).toBe(1);
  });
}

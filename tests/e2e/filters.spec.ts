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

test("filters rendered results by filename", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await page.getByRole("button", { name: "Search" }).click();
  await page.getByLabel("Name or path").fill("sunrise");

  await expect(page.getByText("1 of 2 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expect(page.getByRole("heading", { name: "sunrise.jpg" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeHidden();

  await page.getByRole("button", { name: "Clear 1" }).click();

  await expect(page.getByRole("heading", { name: "portrait.png" })).toBeVisible();
});

test("filters rendered results by metadata fields", async ({ page }) => {
  await mockSearchResponse(page, sortableSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await page.getByLabel("Source type").selectOption("import");
  await expect(page.getByText("2 of 4 result(s), query pHash 0123456789abcdef")).toBeVisible();
  await expectResultOrder(page, ["portrait.png", "clip.mp4"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Duplicate status").selectOption("only");
  await expectResultOrder(page, ["logo.png", "sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Orientation").selectOption("portrait");
  await expectResultOrder(page, ["portrait.png"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Min file size (MB)").fill("2");
  await expectResultOrder(page, ["portrait.png", "clip.mp4"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Media type").selectOption("video_scene");
  await expectResultOrder(page, ["clip.mp4"]);
});

test("applies date, dimension, person, max-size, and duplicate-exclusion filters", async ({
  page,
}) => {
  const filteredSearchResponse = makeSearchResponse({
    count: 4,
    results: [
      makeResult({
        hash_distance: 1,
        image: {
          filename: "alice-landscape.jpg",
          height: 800,
          id: "alice-landscape",
          modified_at: Date.parse("2024-01-10T00:00:00Z") / 1000,
          people: [
            {
              confidence: 0.91,
              face_count: 1,
              label: "Alice",
              media_count: 1,
              person_id: "person-1",
            },
          ],
          relative_path: "people/alice-landscape.jpg",
          size_bytes: 1_500_000,
          width: 1200,
        },
        near_duplicate: false,
      }),
      makeResult({
        hash_distance: 2,
        image: {
          filename: "duplicate-square.jpg",
          height: 512,
          id: "duplicate-square",
          modified_at: Date.parse("2024-02-15T00:00:00Z") / 1000,
          relative_path: "dupes/duplicate-square.jpg",
          size_bytes: 500_000,
          width: 512,
        },
        near_duplicate: true,
      }),
      makeResult({
        hash_distance: 3,
        image: {
          filename: "wide-large.jpg",
          height: 900,
          id: "wide-large",
          modified_at: Date.parse("2024-03-20T00:00:00Z") / 1000,
          relative_path: "wide-large.jpg",
          size_bytes: 6_000_000,
          width: 1600,
        },
        near_duplicate: false,
      }),
      makeResult({
        hash_distance: 4,
        image: {
          filename: "old-portrait.jpg",
          height: 1400,
          id: "old-portrait",
          modified_at: Date.parse("2023-11-01T00:00:00Z") / 1000,
          relative_path: "old-portrait.jpg",
          size_bytes: 2_000_000,
          width: 900,
        },
        near_duplicate: false,
      }),
    ],
  });
  await mockSearchResponseRoute(page, filteredSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await page.getByLabel("Modified after").fill("2024-01-01");
  await page.getByLabel("Modified before").fill("2024-02-29");
  await expectResultOrder(page, ["alice-landscape.jpg", "duplicate-square.jpg"]);

  await page.getByRole("button", { name: "Clear 2" }).click();
  await page.getByLabel("Minimum width").fill("1000");
  await page.getByLabel("Maximum height").fill("900");
  await expectResultOrder(page, ["alice-landscape.jpg", "wide-large.jpg"]);

  await page.getByRole("button", { name: "Clear 2" }).click();
  await page.getByLabel("Person ID").fill("person-1");
  await expectResultOrder(page, ["alice-landscape.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Duplicate status").selectOption("exclude");
  await expectResultOrder(page, ["alice-landscape.jpg", "wide-large.jpg", "old-portrait.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Max file size (MB)").fill("1");
  await expectResultOrder(page, ["duplicate-square.jpg"]);

  await page.getByLabel("Name or path").fill("duplicate");
  await expect(page.getByRole("button", { name: "Clear 2" })).toBeVisible();
  await page.getByRole("button", { name: "Clear 2" }).click();
  await expect(page.getByLabel("Name or path")).toHaveValue("");
  await expect(page.getByLabel("Max file size (MB)")).toHaveValue("");
  await expectResultOrder(page, [
    "alice-landscape.jpg",
    "duplicate-square.jpg",
    "wide-large.jpg",
    "old-portrait.jpg",
  ]);
});

test("displays and filters photo metadata", async ({ page }) => {
  const photoSearchResponse = makeSearchResponse({
    count: 3,
    results: [
      makeResult({
        hash_distance: 1,
        image: {
          filename: "camera-sunrise.jpg",
          id: "camera-sunrise",
          photo_metadata: {
            camera_make: "Acme",
            camera_model: "Pocket 7",
            capture_time: "2024-03-12T10:30:00Z",
            copyright: "Acme Studio",
            creator: "Mira",
            description: "Morning ridge",
            gps: { altitude_meters: 42.4, latitude: 52.5, longitude: 13.4 },
            keywords: ["Travel", "Sunrise"],
            lens_model: "35mm Prime",
            orientation: "Horizontal",
            rating: 4,
            raw: [
              {
                key: "CreateDate",
                label: "CreateDate",
                namespace: "xmp",
                value: "2024-03-12T10:30:00Z",
              },
            ],
            title: "Camera sunrise",
          },
          relative_path: "photos/camera-sunrise.jpg",
        },
      }),
      makeResult({
        hash_distance: 2,
        image: {
          filename: "older-canon.jpg",
          id: "older-canon",
          photo_metadata: {
            camera_make: "Canon",
            camera_model: "R6",
            capture_time: "2023-01-05T08:00:00Z",
            copyright: null,
            creator: null,
            description: null,
            gps: null,
            keywords: ["Portrait"],
            lens_model: "85mm",
            orientation: null,
            rating: null,
            raw: [],
            title: null,
          },
          relative_path: "photos/older-canon.jpg",
        },
      }),
      makeResult({
        hash_distance: 3,
        image: {
          filename: "missing-metadata.jpg",
          id: "missing-metadata",
          photo_metadata: null,
          relative_path: "photos/missing-metadata.jpg",
        },
      }),
    ],
  });
  await mockSearchResponseRoute(page, photoSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  const card = resultCard(page, "camera-sunrise.jpg");
  await expect(card.getByText("Captured")).toBeVisible();
  await expect(card.getByText("Acme Pocket 7")).toBeVisible();
  await expect(card.getByText("35mm Prime")).toBeVisible();
  await expect(card.getByText("52.50000, 13.40000, 42.4 m")).toBeVisible();
  await expect(card.getByText("Travel, Sunrise")).toBeVisible();
  await card.getByText("Photo metadata").click();
  await expect(card.getByText("xmp · CreateDate")).toBeVisible();

  await page.getByLabel("Camera/lens").fill("pocket");
  await expectResultOrder(page, ["camera-sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Keyword").fill("portrait");
  await expectResultOrder(page, ["older-canon.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("GPS metadata").selectOption("yes");
  await expectResultOrder(page, ["camera-sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Captured after").fill("2024-01-01");
  await expectResultOrder(page, ["camera-sunrise.jpg"]);

  await page.getByRole("button", { name: "Clear 1" }).click();
  await page.getByLabel("Sort").selectOption("captured_newest");
  await expectResultOrder(page, ["camera-sunrise.jpg", "older-canon.jpg", "missing-metadata.jpg"]);
});

test("sends OCR and person search parameters and caps filtered candidate limit", async ({
  page,
}) => {
  const personSearchResponse = makeSearchResponse({
    results: [
      makeResult({
        image: {
          filename: "person-match.jpg",
          id: "person-match",
          people: [
            {
              confidence: 0.94,
              face_count: 2,
              label: "Person One",
              media_count: 1,
              person_id: "person-1",
            },
          ],
          photo_metadata: {
            camera_make: "Acme",
            camera_model: "Pocket 7",
            capture_time: null,
            copyright: null,
            creator: null,
            description: null,
            gps: { altitude_meters: null, latitude: 52.5, longitude: 13.4 },
            keywords: [],
            lens_model: null,
            orientation: null,
            rating: null,
            raw: [],
            title: null,
          },
          relative_path: "people/person-match.jpg",
        },
        near_duplicate: false,
      }),
      makeResult({
        image: {
          filename: "other-person.jpg",
          id: "other-person",
          people: [
            {
              confidence: 0.94,
              face_count: 2,
              label: "Person Two",
              media_count: 1,
              person_id: "person-2",
            },
          ],
          relative_path: "people/other-person.jpg",
        },
      }),
    ],
  });
  const capture = await captureSearchRequests(page, personSearchResponse);
  await page.goto("/");

  await page.locator("#query-image").setInputFiles(imageUpload);
  await page.getByLabel("Result limit").fill("100");
  await page.getByLabel("Text query").fill("invoice");
  await page.getByLabel("Person ID").fill("person-1");
  await page.getByLabel("Media type").selectOption("static_image");
  await page.getByLabel("Camera/lens").fill("Pocket");
  await page.getByLabel("Minimum width").fill("640");
  await page.getByLabel("GPS metadata").selectOption("yes");
  await page.getByLabel("Duplicate status").selectOption("exclude");
  await page.getByRole("button", { name: "Search" }).click();

  await expect.poll(() => capture.count).toBe(1);
  await expect
    .poll(() => capture.requests[0])
    .toEqual({
      cameraQuery: "Pocket",
      hasGps: "yes",
      limit: "100",
      mediaKind: "static_image",
      minWidth: "640",
      nearDuplicate: "exclude",
      ocrText: "invoice",
      personId: "person-1",
      sourceType: null,
    });
  await expectResultOrder(page, ["person-match.jpg"]);
});

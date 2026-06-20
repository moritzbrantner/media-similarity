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

test("renders PDF query pages and PDF result metadata", async ({ page }) => {
  await mockSearchResponse(page, pdfSearchResponse);
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: Buffer.from("%PDF-1.4\n"),
    mimeType: "application/pdf",
    name: "query.pdf",
  });
  await expect(page.getByText("PDF query selected")).toBeVisible();

  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByRole("button", { name: "Page 1" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "invoice.pdf page 001" })).toBeVisible();
  await expect(page.locator("span").filter({ hasText: "PDF page" })).toBeVisible();
  await expect(page.getByText("Page 1 of 2")).toBeVisible();
  await expect(page.getByRole("link", { name: "Open PDF" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Open page" })).toBeVisible();
});

test("renders media-specific result cards", async ({ page }) => {
  const mediaSearchResponse = makeSearchResponse({
    count: 5,
    results: [
      makeResult({
        image: {
          animated_thumbnail_url: "/thumbnails/dance-animated.gif",
          filename: "dance.gif",
          frame_count: 12,
          id: "gif-dance",
          media_kind: "animated_gif",
          relative_path: "gifs/dance.gif",
          thumbnail_url: "/thumbnails/dance-still.png",
        },
        near_duplicate: false,
      }),
      makeResult({
        hash_distance: null,
        image: {
          filename: "clip.mp4",
          full_video_url: "/media/clip.mp4",
          height: 1080,
          id: "video-clip",
          media_kind: "video_scene",
          relative_path: "clips/clip.mp4",
          scene_clip_url: "/clips/clip-scene.mp4",
          scene_end_frame: 240,
          scene_end_seconds: 10,
          scene_start_frame: 120,
          scene_start_seconds: 5,
          thumbnail_url: "/thumbnails/clip.png",
          width: 1920,
        },
        near_duplicate: false,
        vector_score: 0.99,
      }),
      makeResult({
        image: {
          audio_analysis: {
            audio_segments: [
              {
                confidence: 0.9,
                end_seconds: 4,
                kind: "speech",
                segment_index: 0,
                speaker_id: "voice-alice",
                speaker_label: "Alice",
                start_seconds: 1,
              },
            ],
            recognized_voices: [
              {
                confidence: 0.93,
                id: "voice-alice",
                label: "Alice",
                segment_count: 1,
                total_seconds: 3,
              },
            ],
            speech_detected: true,
            speech_ratio: 0.75,
            speech_segments: [],
            tempo_bpm: 128.4,
            tempo_confidence: 0.82,
            tempo_onset_count: 12,
            transcript_language: "en",
            transcript_segments: [],
            transcript_text: "hello indexed audio",
          },
          duration_ms: 4000,
          filename: "voice.mp3",
          full_audio_url: "/media/voice.mp3",
          id: "audio-voice",
          media_kind: "audio",
          relative_path: "audio/voice.mp3",
          scene_end_seconds: 4,
          scene_start_seconds: 1,
          thumbnail_url: null,
        },
        near_duplicate: false,
      }),
      makeResult({
        image: {
          faces: [
            {
              bbox: { height: 80, width: 80, x: 10, y: 10 },
              confidence: 0.98,
              face_id: "face-1",
              frame_index: 0,
              media_id: "ocr-face",
              person_id: "person-1",
              person_label: "Ada",
            },
          ],
          filename: "ocr-face.png",
          id: "ocr-face",
          ocr_text: "Conference badge",
          people: [
            {
              confidence: 0.95,
              face_count: 2,
              label: "Ada",
              media_count: 1,
              person_id: "person-1",
            },
          ],
          relative_path: "people/ocr-face.png",
        },
        near_duplicate: false,
        ocr_score: 0.88,
      }),
      makeResult({
        image: {
          filename: "missing-thumb.png",
          id: "missing-thumb",
          relative_path: "missing-thumb.png",
          thumbnail_url: null,
        },
        near_duplicate: false,
      }),
    ],
  });
  await mockSearchResponseRoute(page, mediaSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await expect(resultCard(page, "dance.gif").getByText("GIF", { exact: true })).toBeVisible();
  await expect(resultCard(page, "dance.gif").locator("img")).toHaveAttribute(
    "src",
    "/thumbnails/dance-animated.gif",
  );
  await expect(resultCard(page, "clip.mp4").getByText("Video scene")).toBeVisible();
  await expect(resultCard(page, "clip.mp4").getByText("5.0s-10.0s · frames 120-240")).toBeVisible();
  await expect(
    resultCard(page, "clip.mp4").getByRole("link", { name: "Full video" }),
  ).toBeVisible();
  await expect(
    resultCard(page, "clip.mp4").getByRole("link", { name: "Scene clip" }),
  ).toBeVisible();
  await expect(resultCard(page, "voice.mp3").locator("audio")).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3")
      .locator("span")
      .filter({ hasText: /^Speech$/ }),
  ).toBeVisible();
  await expect(resultCard(page, "voice.mp3").getByText("hello indexed audio")).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3")
      .locator("span")
      .filter({ hasText: /^Alice$/ }),
  ).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3")
      .locator("span")
      .filter({ hasText: /^128 BPM$/ }),
  ).toBeVisible();
  await expect(
    resultCard(page, "voice.mp3").getByRole("link", { name: "Open audio" }),
  ).toBeVisible();
  await expect(resultCard(page, "ocr-face.png").getByText("OCR score")).toBeVisible();
  await expect(resultCard(page, "ocr-face.png").getByText("Conference badge")).toBeVisible();
  await expect(resultCard(page, "ocr-face.png").getByText("Faces 1")).toBeVisible();
  await expect(
    resultCard(page, "ocr-face.png").locator("span").filter({ hasText: /^Ada$/ }),
  ).toBeVisible();
  await expect(resultCard(page, "missing-thumb.png").getByText("Dimensions")).toBeVisible();
  await expect(resultCard(page, "missing-thumb.png").locator("img")).toHaveCount(0);
});

test("metadata-filtered searches keep requested server-side limit", async ({ page }) => {
  let requestedLimit: string | null = null;
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    requestedLimit = new URL(route.request().url()).searchParams.get("limit");
    await route.fulfill({ json: fixtureSearchResponse });
  });
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await page.getByLabel("Result limit").fill("1");
  await page.getByLabel("Media type").selectOption("static_image");
  await page.getByRole("button", { name: "Search" }).click();

  await expect.poll(() => requestedLimit).toBe("1");
  await expect(page.locator("article h3")).toHaveCount(1);
});

test("sorts results by relevance by default and supports changing sort", async ({ page }) => {
  await page.goto("/");

  await page.locator("#query-image").setInputFiles({
    buffer: pngPixel,
    mimeType: "image/png",
    name: "query.png",
  });
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByLabel("Sort")).toHaveValue("relevance");
  await expect(page.locator("article h3")).toHaveText(["sunrise.jpg", "portrait.png"]);

  await page.getByLabel("Sort").selectOption("vector_score");

  await expect(page.locator("article h3")).toHaveText(["portrait.png", "sunrise.jpg"]);
});

test("sorts rendered results with every supported sort mode", async ({ page }) => {
  await mockSearchResponse(page, sortableSearchResponse);
  await page.goto("/");

  await uploadAndSearch(page);

  await expect(page.getByLabel("Sort")).toHaveValue("relevance");
  await expectResultOrder(page, ["logo.png", "sunrise.jpg", "portrait.png", "clip.mp4"]);

  await page.getByLabel("Sort").selectOption("vector_score");
  await expectResultOrder(page, ["clip.mp4", "portrait.png", "logo.png", "sunrise.jpg"]);

  await page.getByLabel("Sort").selectOption("modified_newest");
  await expectResultOrder(page, ["clip.mp4", "sunrise.jpg", "portrait.png", "logo.png"]);

  await page.getByLabel("Sort").selectOption("size_largest");
  await expectResultOrder(page, ["clip.mp4", "portrait.png", "sunrise.jpg", "logo.png"]);

  await page.getByLabel("Sort").selectOption("filename");
  await expectResultOrder(page, ["clip.mp4", "logo.png", "portrait.png", "sunrise.jpg"]);

  await page.getByLabel("Sort").selectOption("phash_distance");
  await expectResultOrder(page, ["logo.png", "sunrise.jpg", "portrait.png", "clip.mp4"]);
});

test("renders and switches video query scenes", async ({ page }) => {
  const sceneOneResult = makeResult({
    image: {
      filename: "first-scene-match.jpg",
      id: "first-scene-match",
      relative_path: "scenes/first.jpg",
    },
  });
  const sceneTwoResult = makeResult({
    image: {
      filename: "second-scene-match.jpg",
      id: "second-scene-match",
      relative_path: "scenes/second.jpg",
    },
  });
  await mockSearchResponseRoute(
    page,
    makeSearchResponse({
      count: 2,
      query_media_kind: "video",
      results: [sceneOneResult, sceneTwoResult],
      scenes: [
        makeScene({
          count: 1,
          end_frame: 48,
          end_seconds: 2,
          results: [sceneOneResult],
          scene_index: 0,
          scene_kind: "video_scene",
          start_frame: 24,
          start_seconds: 1,
        }),
        makeScene({
          clip_url: "/clips/query-scene-2.mp4",
          count: 1,
          end_frame: 96,
          end_seconds: 4,
          results: [sceneTwoResult],
          scene_index: 1,
          scene_kind: "video_scene",
          start_frame: 72,
          start_seconds: 3,
        }),
      ],
    }),
  );
  await page.goto("/");

  await uploadAndSearch(page);

  await expect(page.getByRole("button", { name: /Scene 1 .*1\.0s-2\.0s/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Scene 2 .*3\.0s-4\.0s/ })).toBeVisible();
  await expectResultOrder(page, ["first-scene-match.jpg"]);

  await page.getByRole("button", { name: /Scene 2/ }).click();

  await expect(page.getByRole("link", { name: "Open query clip" })).toBeVisible();
  await expectResultOrder(page, ["second-scene-match.jpg"]);
});

test("renders and switches audio query bits", async ({ page }) => {
  const aliceResult = makeResult({
    image: {
      filename: "alice-voice.mp3",
      id: "alice-voice",
      media_kind: "audio",
      relative_path: "audio/alice.mp3",
    },
  });
  const bobResult = makeResult({
    image: {
      filename: "bob-voice.mp3",
      id: "bob-voice",
      media_kind: "audio",
      relative_path: "audio/bob.mp3",
    },
  });
  await mockSearchResponseRoute(
    page,
    makeSearchResponse({
      count: 2,
      query_media_kind: "audio",
      results: [aliceResult, bobResult],
      scenes: [
        makeScene({
          count: 1,
          end_seconds: 2,
          results: [aliceResult],
          scene_index: 0,
          scene_kind: "audio_bit",
          speaker_id: "voice-alice",
          speaker_label: "Alice",
          start_seconds: 0,
        }),
        makeScene({
          count: 1,
          end_seconds: 5,
          results: [bobResult],
          scene_index: 1,
          scene_kind: "audio_bit",
          speaker_id: "voice-bob",
          speaker_label: "Bob",
          start_seconds: 3,
        }),
      ],
    }),
  );
  await page.goto("/");

  await uploadAndSearch(page);

  await expect(page.getByRole("button", { name: /Bit 1 .* Alice/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Bit 2 .* Bob/ })).toBeVisible();
  await expectResultOrder(page, ["alice-voice.mp3"]);

  await page.getByRole("button", { name: /Bit 2/ }).click();

  await expect(page.getByText("3.0s-5.0s · Bob", { exact: true })).toBeVisible();
  await expectResultOrder(page, ["bob-voice.mp3"]);
});

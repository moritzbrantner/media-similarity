import type { Page } from "@playwright/test";
import {
  completedIndexEvents,
  completedIndexJob,
  healthResponse,
  indexResponse,
  inverseIndexResponse,
  pngPixel,
  searchResponse,
  sourceConfigResponse,
} from "./media-fixtures";

export type ApiMockOptions = {
  health?: unknown;
  models?: unknown;
  sourceConfig?: typeof sourceConfigResponse;
  jobs?: unknown[];
  jobEvents?: unknown[];
  searchResponse?: unknown;
  inverseIndex?: unknown;
};

export type CapturedSearchRequest = {
  limit: string | null;
  ocrText: string | null;
  personId: string | null;
};

export async function installDefaultApiMocks(page: Page, options: ApiMockOptions = {}) {
  let jobs = options.jobs ?? [];
  let currentSourceConfig = options.sourceConfig ?? sourceConfigResponse;
  const cancelledJobIds: string[] = [];
  const deletedMediaIds: string[] = [];
  const sourceConfigPuts: unknown[] = [];
  const indexingConfigPuts: unknown[] = [];

  await page.route("**/api/health", async (route) => {
    await route.fulfill({ json: options.health ?? healthResponse });
  });

  await page.route("**/api/index", async (route) => {
    await route.fulfill({ json: indexResponse });
  });

  await page.route("**/api/inverse-index", async (route) => {
    await route.fulfill({ json: options.inverseIndex ?? inverseIndexResponse });
  });

  await page.route("**/api/jobs/index", async (route) => {
    jobs = [completedIndexJob];
    await route.fulfill({ json: completedIndexJob });
  });

  await page.route("**/api/jobs", async (route) => {
    await route.fulfill({ json: jobs });
  });

  await page.route("**/api/jobs/*/events", async (route) => {
    await route.fulfill({
      json: jobs.length > 0 ? (options.jobEvents ?? completedIndexEvents) : [],
    });
  });

  await page.route("**/api/jobs/*/cancel", async (route) => {
    const match = route
      .request()
      .url()
      .match(/\/api\/jobs\/([^/]+)\/cancel/);
    const jobId = match ? decodeURIComponent(match[1]) : "";
    cancelledJobIds.push(jobId);
    jobs = jobs.map((job) =>
      isJobWithSpec(job) && job.spec.id === jobId
        ? { ...job, finished_at: "2026-05-22T10:00:05Z", status: "Cancelled" }
        : job,
    );
    await route.fulfill({
      json: jobs.find((job) => isJobWithSpec(job) && job.spec.id === jobId) ?? completedIndexJob,
    });
  });

  await page.route("**/api/models", async (route) => {
    await route.fulfill({
      json: options.models ?? {
        models: [
          {
            active: true,
            bundle_path: "/models/visual",
            cached: true,
            configured: "xenova-clip-vit-base-patch32-onnx",
            detail: "Using model bundle `xenova-clip-vit-base-patch32-onnx`",
            label: "Visual embedding",
            options: [],
            role: "visual_embedding",
          },
          {
            active: false,
            bundle_path: null,
            cached: false,
            configured: "base.en",
            detail: "Role is disabled by configuration",
            label: "Audio transcription",
            options: [],
            role: "audio_transcription",
          },
        ],
      },
    });
  });

  await page.route("**/api/models/*/download", async (route) => {
    jobs = [completedIndexJob];
    await route.fulfill({ json: completedIndexJob });
  });

  await page.route("**/api/models/*/enable", async (route) => {
    jobs = [completedIndexJob];
    await route.fulfill({ json: completedIndexJob });
  });

  await page.route("**/api/source-config", async (route) => {
    if (route.request().method() === "PUT") {
      const request = route.request().postDataJSON() as {
        indexing?: typeof sourceConfigResponse.indexing;
        sources?: string[];
      };

      if (request.sources) {
        sourceConfigPuts.push(request);
      }

      if (request.indexing) {
        indexingConfigPuts.push(request);
      }

      currentSourceConfig = {
        ...currentSourceConfig,
        indexing: request.indexing ?? currentSourceConfig.indexing,
        sources:
          request.sources?.map((spec) => ({
            detail: null,
            kind: spec.includes("://") ? spec.split("://")[0] : "local",
            spec,
            status: spec.startsWith("minio:") ? "not_implemented" : "ready",
          })) ?? currentSourceConfig.sources,
      };
      await route.fulfill({ json: currentSourceConfig });
      return;
    }

    await route.fulfill({ json: currentSourceConfig });
  });

  await page.route("**/api/search?**", async (route) => {
    await route.fulfill({ json: options.searchResponse ?? searchResponse });
  });

  await page.route("**/api/indexed-media/*", async (route) => {
    const match = route
      .request()
      .url()
      .match(/\/api\/indexed-media\/([^/?]+)/);
    if (match) {
      deletedMediaIds.push(decodeURIComponent(match[1]));
    }
    await route.fulfill({
      json: { deleted_artifacts: 1, deleted_faces: 0, deleted_points: 1, errors: [] },
    });
  });

  await page.route("**/thumbnails/**", async (route) => {
    await route.fulfill({
      body: pngPixel,
      contentType: "image/png",
    });
  });

  return {
    cancelledJobIds,
    deletedMediaIds,
    indexingConfigPuts,
    sourceConfigPuts,
  };
}

export async function mockSearchResponse(page: Page, response: unknown) {
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    await route.fulfill({ json: response });
  });
}

export async function mockEndpointFailure(
  page: Page,
  pattern: string,
  status: number,
  detail: string,
) {
  await page.unroute(pattern).catch(() => undefined);
  await page.route(pattern, async (route) => {
    await route.fulfill({
      json: { detail },
      status,
    });
  });
}

export async function captureSearchRequests(page: Page, response: unknown) {
  const requests: CapturedSearchRequest[] = [];
  await page.unroute("**/api/search?**");
  await page.route("**/api/search?**", async (route) => {
    const params = new URL(route.request().url()).searchParams;
    requests.push({
      limit: params.get("limit"),
      ocrText: params.get("ocr_text"),
      personId: params.get("person_id"),
    });
    await route.fulfill({ json: response });
  });

  return {
    get count() {
      return requests.length;
    },
    requests,
  };
}

function isJobWithSpec(value: unknown): value is { spec: { id: string } } {
  return Boolean(
    value &&
    typeof value === "object" &&
    "spec" in value &&
    value.spec &&
    typeof value.spec === "object" &&
    "id" in value.spec &&
    typeof value.spec.id === "string",
  );
}

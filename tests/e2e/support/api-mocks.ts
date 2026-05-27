import type { Page } from "@playwright/test";
import {
  completedIndexEvents,
  completedIndexJob,
  healthResponse,
  indexResponse,
  inverseIndexResponse,
  modelsResponse,
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
  cameraQuery: string | null;
  hasGps: string | null;
  limit: string | null;
  mediaKind: string | null;
  minWidth: string | null;
  nearDuplicate: string | null;
  ocrText: string | null;
  personId: string | null;
  sourceType: string | null;
};

export async function installDefaultApiMocks(page: Page, options: ApiMockOptions = {}) {
  let jobs = options.jobs ?? [];
  let currentSourceConfig = options.sourceConfig ?? sourceConfigResponse;
  const cancelledJobIds: string[] = [];
  const deletedMediaIds: string[] = [];
  const mediaTagUpdates: Array<{ id: string; tags: string[] }> = [];
  const modelDownloads: Array<{ model: string | null; role: string }> = [];
  const modelEnables: Array<{ model: string | null; role: string }> = [];
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
      json: options.models ?? modelsResponse,
    });
  });

  await page.route("**/api/models/*/download", async (route) => {
    const role = modelRoleFromUrl(route.request().url(), "download");
    const request = route.request().postDataJSON() as { model?: string | null };
    modelDownloads.push({ model: request.model ?? null, role });
    jobs = [completedIndexJob];
    await route.fulfill({ json: completedIndexJob });
  });

  await page.route("**/api/models/*/enable", async (route) => {
    const role = modelRoleFromUrl(route.request().url(), "enable");
    const request = route.request().postDataJSON() as { model?: string | null };
    modelEnables.push({ model: request.model ?? null, role });
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
            status:
              spec.startsWith("video:") || spec.startsWith("camera:") ? "not_implemented" : "ready",
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

  await page.route("**/api/indexed-media/*/tags", async (route) => {
    const url = route.request().url();
    const tagMatch = url.match(/\/api\/indexed-media\/([^/?]+)\/tags/);
    if (tagMatch && route.request().method() === "PUT") {
      const id = decodeURIComponent(tagMatch[1]);
      const request = route.request().postDataJSON() as { tags?: string[] };
      const tags = request.tags ?? [];
      mediaTagUpdates.push({ id, tags });
      await route.fulfill({
        json: {
          ...findMockImage(id, options.searchResponse ?? searchResponse),
          id,
          tags,
        },
      });
      return;
    }
    await route.fallback();
  });

  await page.route("**/api/indexed-media/*", async (route) => {
    const url = route.request().url();
    const match = url.match(/\/api\/indexed-media\/([^/?]+)/);
    if (match && route.request().method() === "DELETE") {
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
    mediaTagUpdates,
    modelDownloads,
    modelEnables,
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
      cameraQuery: params.get("camera_query"),
      hasGps: params.get("has_gps"),
      limit: params.get("limit"),
      mediaKind: params.get("media_kind"),
      minWidth: params.get("min_width"),
      nearDuplicate: params.get("near_duplicate"),
      ocrText: params.get("ocr_text"),
      personId: params.get("person_id"),
      sourceType: params.get("source_type"),
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

function modelRoleFromUrl(url: string, action: "download" | "enable") {
  const match = url.match(new RegExp(`/api/models/([^/]+)/${action}`));
  return match ? decodeURIComponent(match[1]) : "";
}

function findMockImage(id: string, response: unknown) {
  if (response && typeof response === "object" && "results" in response) {
    const result = (response.results as Array<{ image?: unknown }>).find(
      (item) =>
        item.image &&
        typeof item.image === "object" &&
        "id" in item.image &&
        (item.image as { id?: unknown }).id === id,
    );
    if (result?.image) {
      return result.image;
    }
  }

  return {};
}

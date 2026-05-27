import type { Page, Route } from "@playwright/test";
import {
  completedIndexEvents,
  completedIndexJob,
  healthResponse,
  indexResponse,
  inverseIndexResponse,
  modelsResponse,
  pngPixel,
  searchResponse,
  smartAlbum,
  smartAlbumResults,
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
  smartAlbums?: unknown[];
  smartAlbumResults?: unknown;
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
  const smartAlbumCreates: unknown[] = [];
  const smartAlbumUpdates: Array<{ id: string; request: unknown }> = [];
  const smartAlbumDeletes: string[] = [];
  const identityRenames: Array<{ id: string; kind: "person" | "speaker"; label: string }> = [];
  const identityMerges: Array<{
    kind: "person" | "speaker";
    sourceIds: string[];
    targetId: string;
  }> = [];
  let currentInverseIndex = cloneJson(options.inverseIndex ?? inverseIndexResponse);
  let currentSmartAlbums = cloneJson(options.smartAlbums ?? [smartAlbum]) as unknown[];
  let currentSmartAlbumResults = cloneJson(options.smartAlbumResults ?? smartAlbumResults);

  await page.route("**/api/health", async (route) => {
    await route.fulfill({ json: options.health ?? healthResponse });
  });

  await page.route("**/api/index", async (route) => {
    await route.fulfill({ json: indexResponse });
  });

  await page.route("**/api/inverse-index", async (route) => {
    await route.fulfill({ json: currentInverseIndex });
  });

  await page.route("**/api/smart-albums/preview?**", async (route) => {
    await route.fulfill({
      json: {
        ...(currentSmartAlbumResults as Record<string, unknown>),
        album: {
          ...(route.request().postDataJSON() as Record<string, unknown>),
          created_at: "2026-05-22T10:00:00Z",
          id: "preview",
          updated_at: "2026-05-22T10:00:00Z",
        },
      },
    });
  });

  await page.route("**/api/smart-albums/*/results?**", async (route) => {
    const id = smartAlbumIdFromUrl(route.request().url());
    await route.fulfill({
      json: {
        ...(currentSmartAlbumResults as Record<string, unknown>),
        album:
          currentSmartAlbums.find((album) => isAlbum(album) && album.id === id) ??
          (currentSmartAlbumResults as { album?: unknown }).album,
      },
    });
  });

  await page.route("**/api/smart-albums/*", async (route) => {
    const id = smartAlbumIdFromUrl(route.request().url());
    if (route.request().method() === "PUT") {
      const request = route.request().postDataJSON();
      smartAlbumUpdates.push({ id, request });
      const updated = {
        ...(request as Record<string, unknown>),
        created_at: "2026-05-22T10:00:00Z",
        id,
        updated_at: "2026-05-22T10:01:00Z",
      };
      currentSmartAlbums = currentSmartAlbums.map((album) =>
        isAlbum(album) && album.id === id ? updated : album,
      );
      await route.fulfill({ json: updated });
      return;
    }
    if (route.request().method() === "DELETE") {
      smartAlbumDeletes.push(id);
      currentSmartAlbums = currentSmartAlbums.filter((album) => !isAlbum(album) || album.id !== id);
      await route.fulfill({ json: { deleted: true } });
      return;
    }
    await route.fallback();
  });

  await page.route("**/api/smart-albums", async (route) => {
    if (route.request().method() === "POST") {
      const request = route.request().postDataJSON();
      smartAlbumCreates.push(request);
      const created = {
        ...(request as Record<string, unknown>),
        created_at: "2026-05-22T10:00:00Z",
        id: `album-${currentSmartAlbums.length + 1}`,
        updated_at: "2026-05-22T10:00:00Z",
      };
      currentSmartAlbums = [created, ...currentSmartAlbums];
      await route.fulfill({ json: created });
      return;
    }
    await route.fulfill({ json: { albums: currentSmartAlbums } });
  });

  await page.route("**/api/identities/people/*/merge", async (route) => {
    await handleIdentityMerge(route, "person");
  });

  await page.route("**/api/identities/speakers/*/merge", async (route) => {
    await handleIdentityMerge(route, "speaker");
  });

  await page.route("**/api/identities/people/*", async (route) => {
    await handleIdentityRename(route, "person");
  });

  await page.route("**/api/identities/speakers/*", async (route) => {
    await handleIdentityRename(route, "speaker");
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
    identityMerges,
    identityRenames,
    mediaTagUpdates,
    modelDownloads,
    modelEnables,
    sourceConfigPuts,
    smartAlbumCreates,
    smartAlbumDeletes,
    smartAlbumUpdates,
  };

  async function handleIdentityRename(route: Route, kind: "person" | "speaker") {
    if (route.request().method() !== "PUT") {
      await route.fallback();
      return;
    }
    const id = identityIdFromUrl(route.request().url(), kind, false);
    const request = route.request().postDataJSON() as { label?: string };
    const label = request.label ?? "";
    identityRenames.push({ id, kind, label });
    const entries = identityEntries(currentInverseIndex, kind);
    const target = entries.find((entry) => entry.id === id);
    if (target) {
      target.label = label;
    }
    await route.fulfill({
      json: identityMutationResponse(kind, id, label, [], 1, kind === "person" ? 1 : 0),
    });
  }

  async function handleIdentityMerge(route: Route, kind: "person" | "speaker") {
    if (route.request().method() !== "POST") {
      await route.fallback();
      return;
    }
    const targetId = identityIdFromUrl(route.request().url(), kind, true);
    const request = route.request().postDataJSON() as { source_ids?: string[] };
    const sourceIds = request.source_ids ?? [];
    identityMerges.push({ kind, sourceIds, targetId });
    mergeMockIdentity(currentInverseIndex, kind, targetId, sourceIds);
    const target = identityEntries(currentInverseIndex, kind).find(
      (entry) => entry.id === targetId,
    );
    await route.fulfill({
      json: identityMutationResponse(
        kind,
        targetId,
        target?.label ?? targetId,
        sourceIds,
        1,
        kind === "person" ? sourceIds.length : 0,
      ),
    });
  }
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

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function identityIdFromUrl(url: string, kind: "person" | "speaker", merge: boolean) {
  const segment = kind === "person" ? "people" : "speakers";
  const pattern = merge
    ? new RegExp(`/api/identities/${segment}/([^/]+)/merge`)
    : new RegExp(`/api/identities/${segment}/([^/?]+)`);
  const match = url.match(pattern);
  return match ? decodeURIComponent(match[1]) : "";
}

function identityEntries(index: unknown, kind: "person" | "speaker"): MockIdentityEntry[] {
  if (!index || typeof index !== "object") {
    return [];
  }
  const key = kind === "person" ? "people" : "speakers";
  const entries = (index as Record<string, unknown>)[key];
  return Array.isArray(entries) ? (entries as MockIdentityEntry[]) : [];
}

function mergeMockIdentity(
  index: unknown,
  kind: "person" | "speaker",
  targetId: string,
  sourceIds: string[],
) {
  const entries = identityEntries(index, kind);
  const target = entries.find((entry) => entry.id === targetId);
  if (!target) {
    return;
  }

  const sourceSet = new Set(sourceIds);
  const sources = entries.filter((entry) => sourceSet.has(entry.id));
  for (const source of sources) {
    target.media_count = uniqueLocations([...target.locations, ...source.locations]).length;
    target.locations = uniqueLocations([...target.locations, ...source.locations]);
    if (kind === "person") {
      target.face_count = (target.face_count ?? 0) + (source.face_count ?? 0);
    } else {
      target.segment_count = (target.segment_count ?? 0) + (source.segment_count ?? 0);
      target.total_seconds = roundMillis((target.total_seconds ?? 0) + (source.total_seconds ?? 0));
    }
    target.confidence = Math.max(target.confidence, source.confidence);
  }

  const key = kind === "person" ? "people" : "speakers";
  (index as Record<string, unknown>)[key] = entries.filter((entry) => !sourceSet.has(entry.id));
}

function uniqueLocations(locations: MockIdentityEntry["locations"]) {
  const byMedia = new Map<string, MockIdentityEntry["locations"][number]>();
  for (const location of locations) {
    byMedia.set(location.media_id, location);
  }
  return [...byMedia.values()];
}

function identityMutationResponse(
  kind: "person" | "speaker",
  targetId: string,
  targetLabel: string | null,
  sourceIds: string[],
  updatedMedia: number,
  updatedFaces: number,
) {
  return {
    kind,
    registry_updated: kind === "speaker",
    source_ids: sourceIds,
    target_id: targetId,
    target_label: targetLabel,
    updated_faces: updatedFaces,
    updated_media: updatedMedia,
    warnings: [],
  };
}

function roundMillis(value: number) {
  return Math.round(value * 1000) / 1000;
}

function smartAlbumIdFromUrl(url: string) {
  const match = url.match(/\/api\/smart-albums\/([^/?]+)/);
  return match ? decodeURIComponent(match[1]) : "";
}

function isAlbum(value: unknown): value is { id: string } {
  return Boolean(
    value && typeof value === "object" && "id" in value && typeof value.id === "string",
  );
}

type MockIdentityEntry = {
  confidence: number;
  face_count?: number;
  id: string;
  label: string | null;
  locations: Array<{ media_id: string }>;
  media_count: number;
  segment_count?: number;
  total_seconds?: number;
};

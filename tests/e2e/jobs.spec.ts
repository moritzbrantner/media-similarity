import { expect, test, type Page } from "@playwright/test";

import type { JobSnapshot } from "../../frontend/src/types";
import { makeJob, makeJobEvents } from "./support/media-fixtures";
import { installUiTestMocks, resetApiMocks } from "./support/page-objects";

test.beforeEach(async ({ page }) => {
  await installUiTestMocks(page);
});

test("shows active job progress", async ({ page }) => {
  const runningJob = makeJob({
    finished_at: null,
    progress: {
      completed: 3,
      message: "indexed 3/10 pending source files",
      total: 10,
      unit: "files",
    },
    spec: {
      id: "index.running.mock",
      name: "Index media sources",
    },
    status: "Running",
  });
  await resetApiMocks(page, {
    jobEvents: makeJobEvents(runningJob),
    jobs: [runningJob],
  });
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Index Sources" }).first()).toBeDisabled();
  await expect(page.getByText("indexed 3/10 pending source files").first()).toBeVisible();
  await expect(page.getByText("30%")).toBeVisible();
});

test("keeps indexing cancellation responsive until the job is cancelled", async ({ page }) => {
  const runningJob = makeJob({
    finished_at: null,
    progress: {
      completed: 3,
      message: "indexed 3/10 pending source files",
      total: 10,
      unit: "files",
    },
    spec: {
      id: "index.running.mock",
      name: "Index media sources",
    },
    status: "Running",
  });
  const cancellingJob = makeJob({
    ...runningJob,
    finished_at: null,
    status: "Cancelling",
  });
  const cancelledJob = makeJob({
    ...runningJob,
    finished_at: "2026-05-22T10:00:05Z",
    logs: [
      {
        level: "Warn",
        message: "indexing cancelled while processing /images/large-video.mp4",
        timestamp: "2026-05-22T10:00:05Z",
      },
    ],
    metadata: {
      failed: "0",
      indexed: "3",
      pruned: "0",
      skipped: "1",
    },
    status: "Cancelled",
  });
  let finishCancelRequest!: () => void;
  const cancelRequestCanFinish = new Promise<void>((resolve) => {
    finishCancelRequest = resolve;
  });
  const lifecycle = await installIndexJobCancelLifecycle(page, {
    cancelResponse: cancellingJob,
    cancelRequestCanFinish,
    cancelledJob,
    cancellingJob,
    runningJob,
  });
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Index Sources" }).first()).toBeDisabled();
  await page.getByRole("button", { name: "Cancel" }).click();

  await expect.poll(() => lifecycle.cancelledJobIds).toEqual(["index.running.mock"]);
  await expect(page.getByRole("button", { name: "Cancel" })).toBeDisabled();

  finishCancelRequest();

  await expect(page.getByText("Index media sources · Cancelling")).toBeVisible();
  await expect(page.getByRole("button", { exact: true, name: "Cancelling" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Index Sources" }).first()).toBeDisabled();

  await expect(page.getByText("Index media sources · Cancelled")).toBeVisible();
  await expect(page.getByRole("button", { exact: true, name: "Cancel" })).toBeHidden();
  await expect(page.getByRole("button", { name: "Index Sources" }).first()).toBeEnabled();
});

test("indexes sources from the UI", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Index Sources" }).click();

  await expect(
    page.getByText("Indexed 3 media item(s), skipped 1, pruned 1, failed 0."),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Background Jobs" })).toBeVisible();
  await expect(page.getByText("indexing complete: 3 media item(s)")).toBeVisible();
});

async function installIndexJobCancelLifecycle(
  page: Page,
  {
    cancelRequestCanFinish,
    cancelResponse,
    cancelledJob,
    cancellingJob,
    runningJob,
  }: {
    cancelRequestCanFinish: Promise<void>;
    cancelResponse: JobSnapshot;
    cancelledJob: JobSnapshot;
    cancellingJob: JobSnapshot;
    runningJob: JobSnapshot;
  },
) {
  await resetApiMocks(page, {
    jobEvents: makeJobEvents(runningJob),
    jobs: [runningJob],
  });
  await page.unroute("**/api/jobs");
  await page.unroute("**/api/jobs/*/events");
  await page.unroute("**/api/jobs/*/cancel");

  let cancelRequested = false;
  let jobPollsAfterCancel = 0;
  const cancelledJobIds: string[] = [];

  await page.route("**/api/jobs", async (route) => {
    if (!cancelRequested) {
      await route.fulfill({ json: [runningJob] });
      return;
    }

    jobPollsAfterCancel += 1;
    await route.fulfill({
      json: [jobPollsAfterCancel > 1 ? cancelledJob : cancellingJob],
    });
  });

  await page.route("**/api/jobs/*/events", async (route) => {
    const job =
      jobPollsAfterCancel > 1 ? cancelledJob : cancelRequested ? cancellingJob : runningJob;
    await route.fulfill({ json: makeJobEvents(job) });
  });

  await page.route("**/api/jobs/*/cancel", async (route) => {
    const match = route
      .request()
      .url()
      .match(/\/api\/jobs\/([^/]+)\/cancel/);
    cancelledJobIds.push(match ? decodeURIComponent(match[1]) : "");
    cancelRequested = true;
    await cancelRequestCanFinish;
    await route.fulfill({ json: cancelResponse });
  });

  return {
    cancelledJobIds,
  };
}

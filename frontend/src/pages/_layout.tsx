import { useMemo } from "react";
import { Outlet, useOutletContext } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { AppHeader } from "../components/app-header";
import { JobsPanel } from "../components/jobs-panel";
import { useAlbumController } from "../features/albums/useAlbumController";
import { useJobsController } from "../features/jobs/useJobsController";
import { fetchHealth } from "../api";
import type { EditableSmartAlbum, HealthResponse, IndexResponse } from "../types";
import type { MetadataFilters, ResultSortMode } from "../search/types";

export type AppShellContext = {
  albumDraft: EditableSmartAlbum | null;
  beginAlbumFromSearch: (params: {
    filters: MetadataFilters;
    limit: number;
    ocrTextQuery: string;
    sortMode: ResultSortMode;
  }) => void;
  consumeAlbumDraft: () => void;
  health: HealthResponse | undefined;
  indexActive: boolean;
  indexError: Error | null;
  indexPending: boolean;
  lastIndex: IndexResponse | null;
  onIndex: () => void;
};

export function AppShell() {
  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
  });
  const jobs = useJobsController({
    healthData: healthQuery.data,
  });
  const albumController = useAlbumController();

  const sourceList = useMemo(() => {
    if (!healthQuery.data) {
      return healthQuery.isError ? "Service is not responding" : "Checking service status";
    }

    const sources =
      healthQuery.data.sources.length > 0
        ? healthQuery.data.sources
        : [healthQuery.data.source_dir];
    return sources.join(", ");
  }, [healthQuery.data, healthQuery.isError]);

  const context: AppShellContext = {
    albumDraft: albumController.albumDraft,
    beginAlbumFromSearch: albumController.beginAlbumFromSearch,
    consumeAlbumDraft: albumController.consumeAlbumDraft,
    health: healthQuery.data,
    indexActive: jobs.indexActive,
    indexError: jobs.indexError as Error | null,
    indexPending: jobs.indexMutation.isPending,
    lastIndex: jobs.lastIndex,
    onIndex: () => jobs.indexMutation.mutate(),
  };

  return (
    <main className="min-h-screen bg-neutral-100 text-neutral-950">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-5 sm:px-6 lg:px-8">
        <AppHeader
          health={healthQuery.data}
          healthError={healthQuery.isError}
          healthLoading={healthQuery.isLoading}
          indexActive={jobs.indexActive}
          indexPending={jobs.indexMutation.isPending}
          onIndex={() => jobs.indexMutation.mutate()}
          sourcesLabel={sourceList}
        />

        <JobsPanel
          cancelPendingJobId={jobs.cancelJobMutation.variables ?? null}
          error={jobs.jobsQuery.error}
          events={jobs.jobEventsQuery.data ?? []}
          jobs={jobs.jobs}
          onCancel={(jobId) => jobs.cancelJobMutation.mutate(jobId)}
          onSelectJob={jobs.setSelectedJobId}
          selectedJobId={jobs.selectedJob?.spec.id ?? null}
        />

        <Outlet context={context} />
      </div>
    </main>
  );
}

export function useAppShellContext() {
  return useOutletContext<AppShellContext>();
}

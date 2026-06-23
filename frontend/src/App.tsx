import { Suspense, lazy, useMemo, useState } from "react";
import { Loader2 } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AppHeader } from "./components/app-header";
import { JobsPanel } from "./components/jobs-panel";
import { useAlbumController } from "./features/albums/useAlbumController";
import { useJobsController } from "./features/jobs/useJobsController";
import { useSearchController } from "./features/search/useSearchController";
import { useConfigurationController } from "./features/configuration/useConfigurationController";
import { useWorkflowsController } from "./features/workflows/useWorkflowsController";
import { SearchPage } from "./features/search/pages/search-page";
import { SmartAlbumsPage } from "./features/albums/pages/smart-albums-page";
import { SourceConfigurationPage } from "./features/configuration/pages/source-configuration-page";
import { WorkflowConfigurationPage } from "./features/workflows/pages/workflow-configuration-page";
import type { IndexResponse } from "./types";
import { fetchHealth, fetchInverseIndex, mergeIdentities, renameIdentity } from "./api";
import type { IdentityKind } from "./api";
import type { AppView } from "./search/types";

const InverseIndexPage = lazy(() =>
  import("./components/inverse-index-page").then((module) => ({
    default: module.InverseIndexPage,
  })),
);

export function App() {
  const queryClient = useQueryClient();
  const [activeView, setActiveView] = useState<AppView>("search");

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
  });

  const jobs = useJobsController({
    healthData: healthQuery.data,
  });
  const searchController = useSearchController();
  const searchHistory = searchController.searchHistory;

  const configController = useConfigurationController({
    sourceConfigEnabled: activeView === "configure",
  });

  const workflowController = useWorkflowsController({
    workflowsEnabled: activeView === "workflows",
  });
  const inverseIndexQuery = useQuery({
    queryKey: ["inverse-index"],
    queryFn: fetchInverseIndex,
    enabled: activeView === "inverse-index",
  });

  const albumController = useAlbumController();

  const indexMutation = jobs.indexMutation;
  const cancelJobMutation = jobs.cancelJobMutation;
  const jobsQuery = jobs.jobsQuery;
  const jobsList = jobs.jobs;
  const latestIndexJob = jobs.latestIndexJob;
  const selectedJob = jobs.selectedJob;

  const modelsError = configController.modelError;
  const sourceConfigQuery = configController.sourceConfigQuery;
  const modelsQuery = configController.modelsQuery;
  const sourceConfigMutation = configController.sourceConfigMutation;

  const jobEventsQuery = jobs.jobEventsQuery;

  const configurationModelActionPending = configController.modelActionPending;

  const lastIndex = jobs.lastIndex as IndexResponse | null;

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

  const indexPending = indexMutation.isPending;

  const sourceKindMutation = useMutation({
    mutationFn: ({ id, kind, label }: { id: string; kind: IdentityKind; label: string }) =>
      renameIdentity(kind, id, label),
    onSuccess: (response) => {
      searchController.applyIdentityMutationToSearchHistory(response);
      invalidateIdentityQueries(queryClient);
    },
  });

  const mergeIdentitiesMutation = useMutation({
    mutationFn: ({
      kind,
      sourceIds,
      targetId,
    }: {
      kind: IdentityKind;
      sourceIds: string[];
      targetId: string;
    }) => mergeIdentities(kind, targetId, sourceIds),
    onSuccess: (response) => {
      searchController.applyIdentityMutationToSearchHistory(response);
      invalidateIdentityQueries(queryClient);
    },
  });

  const indexActive = Boolean(latestIndexJob && indexPending);

  const activeSearch = searchController.activeSearch;
  const activeResponse = searchController.activeResponse;

  const sourceTypeOptions = searchController.sourceTypeOptions;
  const results = searchController.results;

  const indexError = searchController.searchError;
  const deletePendingId = searchController.deletePendingId;

  return (
    <main className="min-h-screen bg-neutral-100 text-neutral-950">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-5 sm:px-6 lg:px-8">
        <AppHeader
          activeView={activeView}
          health={healthQuery.data}
          healthError={healthQuery.isError}
          healthLoading={healthQuery.isLoading}
          indexActive={Boolean(jobs.latestIndexJob && jobs.latestIndexJob.status === "Running")}
          indexPending={indexMutation.isPending}
          onIndex={() => indexMutation.mutate()}
          onViewChange={setActiveView}
          sourcesLabel={sourceList}
        />

        <JobsPanel
          cancelPendingJobId={cancelJobMutation.variables ?? null}
          error={jobsQuery.error}
          events={jobEventsQuery.data ?? []}
          jobs={jobsList}
          onCancel={(jobId) => cancelJobMutation.mutate(jobId)}
          onSelectJob={jobs.setSelectedJobId}
          selectedJobId={selectedJob?.spec.id ?? null}
        />

        {activeView === "search" ? (
          <SearchPage
            activeResponse={activeResponse}
            activeSearch={activeSearch}
            activeSearchId={searchController.activeSearchId}
            deletingId={deletePendingId}
            displayedPreviewUrl={searchController.displayedPreviewUrl}
            file={searchController.file}
            faceResponse={searchController.faceResponse}
            health={healthQuery.data}
            indexError={indexError}
            lastIndex={lastIndex}
            limit={searchController.limit}
            metadataFilters={searchController.metadataFilters}
            ocrTextQuery={searchController.ocrTextQuery}
            onDelete={(id) => searchController.deleteMediaMutation.mutate(id)}
            onFileChange={searchController.handleFileChange}
            onHistorySelect={searchController.handleHistorySelect}
            onLimitChange={searchController.handleLimitChange}
            onMetadataFiltersChange={searchController.handleMetadataFiltersChange}
            onOcrTextQueryChange={searchController.setOcrTextQuery}
            onResultSortModeChange={searchController.handleResultSortModeChange}
            onSearchModeChange={searchController.setSearchMode}
            onSaveAsAlbum={() => {
              albumController.beginAlbumFromSearch({
                filters: searchController.metadataFilters,
                limit: searchController.limit,
                ocrTextQuery: searchController.ocrTextQuery,
                sortMode: searchController.resultSortMode,
              });
              setActiveView("albums");
            }}
            onSearchSubmit={searchController.handleSubmit}
            onSelectQueryScene={searchController.setSelectedQuerySceneIndex}
            onUpdateTags={(id, tags) =>
              searchController.updateMediaTagsMutation.mutate({
                id,
                tags,
              })
            }
            previewIsAudio={searchController.previewIsAudio}
            previewIsPdf={searchController.previewIsPdf}
            previewIsText={searchController.previewIsText}
            previewIsVideo={searchController.previewIsVideo}
            resultSortMode={searchController.resultSortMode}
            results={results}
            searchError={searchController.searchError}
            searchHistory={searchHistory}
            searchMode={searchController.searchMode}
            searchPending={searchController.searchPending}
            selectedQuerySceneIndex={searchController.selectedQuerySceneIndex}
            showMetadataFilters={searchController.showMetadataFilters}
            sourceTypeOptions={sourceTypeOptions}
            tagSavingId={
              searchController.updateMediaTagsMutation.isPending
                ? searchController.updateMediaTagsMutation.variables?.id
                : undefined
            }
          />
        ) : (
          <Suspense
            fallback={
              <section className="flex min-h-72 items-center justify-center rounded-lg border border-neutral-300 bg-white text-sm font-medium text-neutral-600 shadow-sm">
                <Loader2 className="mr-2 size-4 animate-spin" aria-hidden="true" />
                Loading view
              </section>
            }
          >
            {activeView === "albums" ? (
              <SmartAlbumsPage
                initialDraft={albumController.albumDraft}
                onDraftConsumed={() => albumController.consumeAlbumDraft()}
              />
            ) : activeView === "inverse-index" ? (
              <InverseIndexPage
                data={inverseIndexQuery.data ?? null}
                error={inverseIndexQuery.error}
                loading={inverseIndexQuery.isLoading}
                mergeError={mergeIdentitiesMutation.error}
                mergeErrorIdentity={
                  mergeIdentitiesMutation.isError && mergeIdentitiesMutation.variables
                    ? {
                        id: mergeIdentitiesMutation.variables.targetId,
                        kind: mergeIdentitiesMutation.variables.kind,
                      }
                    : null
                }
                mergingIdentity={
                  mergeIdentitiesMutation.isPending && mergeIdentitiesMutation.variables
                    ? {
                        id: mergeIdentitiesMutation.variables.targetId,
                        kind: mergeIdentitiesMutation.variables.kind,
                      }
                    : null
                }
                onMergeIdentity={(kind, targetId, sourceIds) =>
                  mergeIdentitiesMutation.mutateAsync({
                    kind,
                    sourceIds,
                    targetId,
                  })
                }
                onRefresh={() => {
                  inverseIndexQuery.refetch();
                }}
                onRenameIdentity={(kind, id, label) =>
                  sourceKindMutation.mutateAsync({ id, kind, label })
                }
                refreshing={inverseIndexQuery.isFetching}
                renameError={sourceKindMutation.error}
                renameErrorIdentity={
                  sourceKindMutation.isError && sourceKindMutation.variables
                    ? {
                        id: sourceKindMutation.variables.id,
                        kind: sourceKindMutation.variables.kind,
                      }
                    : null
                }
                renamingIdentity={
                  sourceKindMutation.isPending && sourceKindMutation.variables
                    ? {
                        id: sourceKindMutation.variables.id,
                        kind: sourceKindMutation.variables.kind,
                      }
                    : null
                }
              />
            ) : activeView === "configure" ? (
              <SourceConfigurationPage
                config={sourceConfigQuery.data ?? null}
                error={sourceConfigQuery.error}
                indexError={jobs.indexError}
                indexPending={indexMutation.isPending || indexActive}
                lastIndex={lastIndex}
                loading={sourceConfigQuery.isLoading}
                modelActionPending={configurationModelActionPending}
                modelError={(modelsError as Error | null) ?? null}
                models={modelsQuery.data ?? null}
                modelsError={modelsQuery.error}
                modelsLoading={modelsQuery.isLoading}
                onDownloadAllModels={() => configController.downloadAllModelsMutation.mutate()}
                onDownloadModel={(role, model) =>
                  configController.downloadModelMutation.mutate({ role, model })
                }
                onDisableModel={(role) => configController.disableModelMutation.mutate({ role })}
                onEnableModel={(role, model) =>
                  configController.enableModelMutation.mutate({ role, model })
                }
                onIndex={() => indexMutation.mutate()}
                onSave={(sources) => sourceConfigMutation.mutate(sources)}
                saveError={sourceConfigMutation.error}
                savePending={sourceConfigMutation.isPending}
                saveSuccess={sourceConfigMutation.isSuccess}
              />
            ) : activeView === "workflows" ? (
              <WorkflowConfigurationPage
                config={workflowController.workflowsQuery.data ?? null}
                error={workflowController.workflowsQuery.error}
                indexError={jobs.indexError}
                indexPending={indexMutation.isPending || indexActive}
                lastIndex={lastIndex}
                loading={workflowController.workflowsQuery.isLoading}
                onIndex={() => indexMutation.mutate()}
                onReset={() => workflowController.workflowResetMutation.mutate()}
                onSave={(library) => workflowController.workflowMutation.mutate(library)}
                onValidate={(library) =>
                  workflowController.workflowValidateMutation
                    .mutateAsync(library)
                    .then((response) => response.diagnostics)
                }
                resetPending={workflowController.workflowResetMutation.isPending}
                saveError={workflowController.workflowMutation.error}
                savePending={workflowController.workflowMutation.isPending}
                saveSuccess={workflowController.workflowMutation.isSuccess}
                validateError={workflowController.workflowValidateMutation.error}
                validatePending={workflowController.workflowValidateMutation.isPending}
              />
            ) : null}
          </Suspense>
        )}
      </div>
    </main>
  );
}

function invalidateIdentityQueries(queryClient: ReturnType<typeof useQueryClient>) {
  queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
}

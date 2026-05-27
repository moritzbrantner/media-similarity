import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import type { FormEvent } from "react";
import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import {
  cancelJob,
  deleteIndexedMedia,
  downloadModel,
  enableModel,
  fetchHealth,
  fetchInverseIndex,
  fetchJobEvents,
  fetchJobs,
  fetchModels,
  fetchSourceConfig,
  mergeIdentities,
  renameIdentity,
  searchMedia,
  startIndexJob,
  updateIndexedMediaTags,
  updateIndexingConfig,
  updateSourceConfig,
} from "./api";
import type { IdentityKind } from "./api";
import type { IdentityMutationResponse } from "./api";
import { AppHeader } from "./components/app-header";
import { JobsPanel } from "./components/jobs-panel";
import { jobIsActive, jobIsTerminal, numberFromMetadata, sortJobs } from "./jobs/job-utils";
import { isAudioFile, isPdfFile } from "./lib/media";
import {
  DEFAULT_LIMIT,
  DEFAULT_METADATA_FILTERS,
  DEFAULT_RESULT_SORT,
  MAX_SEARCH_HISTORY,
  SEARCH_HISTORY_QUERY_KEY,
} from "./search/defaults";
import { filterResults, sourceTypesFor } from "./search/filtering";
import {
  loadSearchHistory,
  removeResultFromResponse,
  saveSearchHistory,
  updateMediaInResponse,
} from "./search/history";
import { createQueryPreview } from "./search/preview";
import { sortResults } from "./search/sorting";
import type {
  AppView,
  MetadataFilters,
  ResultSortMode,
  SearchHistoryItem,
  SearchVariables,
} from "./search/types";
import { SearchPage } from "./views/search-page";
import type { IndexResponse, SearchResult } from "./types";

const InverseIndexPage = lazy(() =>
  import("./components/inverse-index-page").then((module) => ({
    default: module.InverseIndexPage,
  })),
);

const SourceConfigurationPage = lazy(() =>
  import("./components/source-configuration-page").then((module) => ({
    default: module.SourceConfigurationPage,
  })),
);

const IndexingConfigurationPage = lazy(() =>
  import("./components/indexing-configuration-page").then((module) => ({
    default: module.IndexingConfigurationPage,
  })),
);

export function App() {
  const queryClient = useQueryClient();
  const [activeView, setActiveView] = useState<AppView>("search");
  const [file, setFile] = useState<File | null>(null);
  const [limit, setLimit] = useState(DEFAULT_LIMIT);
  const [metadataFilters, setMetadataFilters] = useState<MetadataFilters>(DEFAULT_METADATA_FILTERS);
  const [ocrTextQuery, setOcrTextQuery] = useState("");
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [resultSortMode, setResultSortMode] = useState<ResultSortMode>(DEFAULT_RESULT_SORT);
  const [lastIndex, setLastIndex] = useState<IndexResponse | null>(null);
  const [activeSearchId, setActiveSearchId] = useState<string | null>(null);
  const [selectedQuerySceneIndex, setSelectedQuerySceneIndex] = useState<number | null>(null);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [refreshedModelJobId, setRefreshedModelJobId] = useState<string | null>(null);
  const sourceConfigViewActive = activeView === "configure" || activeView === "indexing";

  const searchHistoryQuery = useQuery({
    queryKey: SEARCH_HISTORY_QUERY_KEY,
    queryFn: loadSearchHistory,
    initialData: loadSearchHistory,
    staleTime: Infinity,
  });

  const searchHistory = searchHistoryQuery.data;

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
  });

  const sourceConfigQuery = useQuery({
    queryKey: ["source-config"],
    queryFn: fetchSourceConfig,
    enabled: sourceConfigViewActive,
  });

  const inverseIndexQuery = useQuery({
    queryKey: ["inverse-index"],
    queryFn: fetchInverseIndex,
    enabled: activeView === "inverse-index",
  });

  const modelsQuery = useQuery({
    queryKey: ["models"],
    queryFn: fetchModels,
    enabled: activeView === "configure",
  });

  const jobsQuery = useQuery({
    queryKey: ["jobs"],
    queryFn: fetchJobs,
    refetchInterval: 2000,
  });

  const jobs = useMemo(() => sortJobs(jobsQuery.data ?? []), [jobsQuery.data]);
  const selectedJob = jobs.find((job) => job.spec.id === selectedJobId) ?? jobs[0] ?? null;
  const latestIndexJob = jobs.find((job) => job.spec.kind?.startsWith("index."));
  const latestModelJob = jobs.find((job) => job.spec.kind?.startsWith("model."));

  const jobEventsQuery = useQuery({
    queryKey: ["job-events", selectedJob?.spec.id],
    queryFn: () => fetchJobEvents(selectedJob?.spec.id ?? ""),
    enabled: Boolean(selectedJob),
    refetchInterval: selectedJob && jobIsActive(selectedJob) ? 1500 : false,
  });

  const indexMutation = useMutation({
    mutationFn: startIndexJob,
    onSuccess: (job) => {
      setSelectedJobId(job.spec.id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
  });

  const cancelJobMutation = useMutation({
    mutationFn: cancelJob,
    onSuccess: (job) => {
      setSelectedJobId(job.spec.id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["job-events", job.spec.id] });
    },
  });

  const downloadModelMutation = useMutation({
    mutationFn: ({ model, role }: { model?: string | null; role: string }) =>
      downloadModel(role, model),
    onSuccess: (job) => {
      setSelectedJobId(job.spec.id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["models"] });
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const enableModelMutation = useMutation({
    mutationFn: ({ model, role }: { model?: string | null; role: string }) =>
      enableModel(role, model),
    onSuccess: (job) => {
      setSelectedJobId(job.spec.id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["models"] });
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const deleteMediaMutation = useMutation({
    mutationFn: deleteIndexedMedia,
    onSuccess: (_response, id) => {
      removeMediaFromSearchHistory(id);
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
    },
  });

  const updateMediaTagsMutation = useMutation({
    mutationFn: updateIndexedMediaTags,
    onSuccess: (media) => {
      updateMediaInSearchHistory(media);
      queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
    },
  });

  const sourceConfigMutation = useMutation({
    mutationFn: updateSourceConfig,
    onSuccess: (response) => {
      queryClient.setQueryData(["source-config"], response);
      queryClient.invalidateQueries({ queryKey: ["health"] });
    },
  });

  const indexingConfigMutation = useMutation({
    mutationFn: updateIndexingConfig,
    onSuccess: (response) => {
      queryClient.setQueryData(["source-config"], response);
      queryClient.invalidateQueries({ queryKey: ["health"] });
    },
  });

  const renameIdentityMutation = useMutation({
    mutationFn: ({ id, kind, label }: { id: string; kind: IdentityKind; label: string }) =>
      renameIdentity(kind, id, label),
    onSuccess: (response) => {
      applyIdentityMutationToSearchHistory(response);
      invalidateIdentityQueries();
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
      applyIdentityMutationToSearchHistory(response);
      invalidateIdentityQueries();
    },
  });

  const searchMutation = useMutation({
    mutationFn: ({ filters, ocrTextQuery, queryFile, resultLimit }: SearchVariables) =>
      searchMedia(queryFile, resultLimit, ocrTextQuery, filters),
    onSuccess: (response, variables) => {
      const nextItem: SearchHistoryItem = {
        id: createHistoryId(),
        fileName: variables.queryFile.name,
        filters: variables.filters,
        limit: variables.resultLimit,
        ocrTextQuery: variables.ocrTextQuery,
        queryImageUrl: variables.queryImageUrl,
        queryMediaKind: response.query_media_kind,
        sortMode: variables.sortMode,
        searchedAt: new Date().toISOString(),
        response,
      };

      updateSearchHistory((history) => [nextItem, ...history].slice(0, MAX_SEARCH_HISTORY));
      setActiveSearchId(nextItem.id);
      setSelectedQuerySceneIndex(response.scenes[0]?.scene_index ?? null);
    },
  });

  useEffect(() => {
    if (!file || isPdfFile(file)) {
      setPreviewUrl(null);
      return;
    }

    const url = URL.createObjectURL(file);
    setPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [file]);

  useEffect(() => {
    saveSearchHistory(searchHistory);
  }, [searchHistory]);

  useEffect(() => {
    if (!selectedJobId && jobs.length > 0) {
      setSelectedJobId(jobs[0].spec.id);
    }
  }, [jobs, selectedJobId]);

  useEffect(() => {
    if (!latestIndexJob || !jobIsTerminal(latestIndexJob.status)) {
      return;
    }

    const indexed = numberFromMetadata(latestIndexJob.metadata.indexed);
    const skipped = numberFromMetadata(latestIndexJob.metadata.skipped);
    const failed = numberFromMetadata(latestIndexJob.metadata.failed);
    if (indexed === null || skipped === null || failed === null) {
      return;
    }

    setLastIndex({
      collection: latestIndexJob.metadata.collection ?? healthQuery.data?.collection ?? "",
      errors: latestIndexJob.logs
        .filter((entry) => entry.level === "Warn" || entry.level === "Error")
        .map((entry) => entry.message),
      failed,
      indexed,
      pruned: numberFromMetadata(latestIndexJob.metadata.pruned) ?? 0,
      skipped,
      source_dir: healthQuery.data?.source_dir ?? "",
      sources: healthQuery.data?.sources ?? [],
    });

    if (latestIndexJob.status === "Succeeded") {
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
    }
  }, [healthQuery.data, latestIndexJob, queryClient]);

  useEffect(() => {
    if (
      !latestModelJob ||
      !jobIsTerminal(latestModelJob.status) ||
      latestModelJob.spec.id === refreshedModelJobId
    ) {
      return;
    }

    setRefreshedModelJobId(latestModelJob.spec.id);
    queryClient.invalidateQueries({ queryKey: ["models"] });
    queryClient.invalidateQueries({ queryKey: ["health"] });
    queryClient.invalidateQueries({ queryKey: ["source-config"] });
  }, [latestModelJob, queryClient, refreshedModelJobId]);

  function updateSearchHistory(updater: (history: SearchHistoryItem[]) => SearchHistoryItem[]) {
    queryClient.setQueryData<SearchHistoryItem[]>(SEARCH_HISTORY_QUERY_KEY, (history = []) =>
      updater(history),
    );
  }

  function updateActiveSearch(updater: (item: SearchHistoryItem) => SearchHistoryItem) {
    if (!activeSearchId) {
      return;
    }

    updateSearchHistory((history) =>
      history.map((item) => (item.id === activeSearchId ? updater(item) : item)),
    );
  }

  function removeMediaFromSearchHistory(id: string) {
    updateSearchHistory((history) =>
      history.map((item) => ({
        ...item,
        response: removeResultFromResponse(item.response, id),
      })),
    );
  }

  function updateMediaInSearchHistory(media: SearchResult["image"]) {
    updateSearchHistory((history) =>
      history.map((item) => ({
        ...item,
        response: updateMediaInResponse(item.response, media),
      })),
    );
  }

  function applyIdentityMutationToSearchHistory(mutation: IdentityMutationResponse) {
    updateSearchHistory((history) =>
      history.map((item) => ({
        ...item,
        response: {
          ...item.response,
          results: item.response.results.map((result) => ({
            ...result,
            image:
              mutation.kind === "person"
                ? applyPersonMutation(result.image, mutation)
                : applySpeakerMutation(result.image, mutation),
          })),
        },
      })),
    );
  }

  function invalidateIdentityQueries() {
    queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
    if (activeResponse) {
      queryClient.invalidateQueries({ queryKey: ["search"] });
    }
  }

  const sourcesLabel = useMemo(() => {
    const health = healthQuery.data;
    if (!health) {
      return healthQuery.isError ? "Service is not responding" : "Checking service status";
    }

    const sources = health.sources.length > 0 ? health.sources : [health.source_dir];
    return sources.join(", ");
  }, [healthQuery.data, healthQuery.isError]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!file) {
      return;
    }

    setActiveSearchId(null);
    const queryImageUrl =
      file.type.startsWith("video/") || isAudioFile(file) || isPdfFile(file)
        ? previewUrl
        : await createQueryPreview(file);
    searchMutation.mutate({
      filters: metadataFilters,
      ocrTextQuery,
      queryFile: file,
      queryImageUrl,
      resultLimit: limit,
      sortMode: resultSortMode,
    });
  }

  function handleFileChange(nextFile: File | null) {
    setFile(nextFile);
    setActiveSearchId(null);
    setSelectedQuerySceneIndex(null);
    searchMutation.reset();
  }

  function handleLimitChange(value: string) {
    const nextLimit = Number(value || DEFAULT_LIMIT);
    setLimit(nextLimit);
    updateActiveSearch((item) => ({ ...item, limit: nextLimit }));
  }

  function handleMetadataFiltersChange(filters: MetadataFilters) {
    setMetadataFilters(filters);
    updateActiveSearch((item) => ({ ...item, filters }));
  }

  function handleResultSortModeChange(sortMode: ResultSortMode) {
    setResultSortMode(sortMode);
    updateActiveSearch((item) => ({ ...item, sortMode }));
  }

  const activeSearch = searchHistory.find((item) => item.id === activeSearchId) ?? null;
  const activeResponse = activeSearch?.response ?? null;
  const displayedPreviewUrl = activeSearch ? activeSearch.queryImageUrl : previewUrl;
  const previewIsVideo = activeSearch
    ? activeSearch.queryMediaKind === "video"
    : Boolean(file?.type.startsWith("video/"));
  const previewIsAudio = activeSearch
    ? activeSearch.queryMediaKind === "audio"
    : Boolean(file && isAudioFile(file));
  const previewIsPdf = activeSearch
    ? activeSearch.queryMediaKind === "pdf"
    : Boolean(file && isPdfFile(file));
  const showMetadataFilters = Boolean(file || activeSearch);
  const sourceTypeOptions = sourceTypesFor(
    activeResponse?.results ?? [],
    metadataFilters.sourceType,
  );
  const filteredResults = sortResults(
    filterResults(activeResponse?.results ?? [], metadataFilters),
    resultSortMode,
  );
  const results = filteredResults.slice(0, activeSearch?.limit ?? limit);
  const indexActive = Boolean(latestIndexJob && jobIsActive(latestIndexJob));

  function handleHistorySelect(item: SearchHistoryItem) {
    setActiveSearchId(item.id);
    setLimit(item.limit);
    setMetadataFilters(item.filters);
    setOcrTextQuery(item.ocrTextQuery);
    setResultSortMode(item.sortMode);
    setSelectedQuerySceneIndex(item.response.scenes[0]?.scene_index ?? null);
    searchMutation.reset();
  }

  return (
    <main className="min-h-screen bg-neutral-100 text-neutral-950">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-5 sm:px-6 lg:px-8">
        <AppHeader
          activeView={activeView}
          health={healthQuery.data}
          healthError={healthQuery.isError}
          healthLoading={healthQuery.isLoading}
          indexActive={indexActive}
          indexPending={indexMutation.isPending}
          onIndex={() => indexMutation.mutate()}
          onViewChange={setActiveView}
          sourcesLabel={sourcesLabel}
        />

        <JobsPanel
          cancelPendingJobId={cancelJobMutation.variables ?? null}
          error={jobsQuery.error}
          events={jobEventsQuery.data ?? []}
          jobs={jobs}
          onCancel={(jobId) => cancelJobMutation.mutate(jobId)}
          onSelectJob={setSelectedJobId}
          selectedJobId={selectedJob?.spec.id ?? null}
        />

        {activeView === "search" ? (
          <SearchPage
            activeResponse={activeResponse}
            activeSearch={activeSearch}
            activeSearchId={activeSearchId}
            deletingId={
              deleteMediaMutation.isPending
                ? (deleteMediaMutation.variables as string | undefined)
                : undefined
            }
            displayedPreviewUrl={displayedPreviewUrl}
            file={file}
            health={healthQuery.data}
            indexError={indexMutation.error}
            lastIndex={lastIndex}
            limit={limit}
            metadataFilters={metadataFilters}
            ocrTextQuery={ocrTextQuery}
            onDelete={(id) => deleteMediaMutation.mutate(id)}
            onFileChange={handleFileChange}
            onHistorySelect={handleHistorySelect}
            onLimitChange={handleLimitChange}
            onMetadataFiltersChange={handleMetadataFiltersChange}
            onOcrTextQueryChange={setOcrTextQuery}
            onResultSortModeChange={handleResultSortModeChange}
            onSearchSubmit={handleSubmit}
            onSelectQueryScene={setSelectedQuerySceneIndex}
            onUpdateTags={(id, tags) => updateMediaTagsMutation.mutate({ id, tags })}
            previewIsAudio={previewIsAudio}
            previewIsPdf={previewIsPdf}
            previewIsVideo={previewIsVideo}
            resultSortMode={resultSortMode}
            results={results}
            searchError={searchMutation.error}
            searchHistory={searchHistory}
            searchPending={searchMutation.isPending}
            selectedQuerySceneIndex={selectedQuerySceneIndex}
            showMetadataFilters={showMetadataFilters}
            sourceTypeOptions={sourceTypeOptions}
            tagSavingId={
              updateMediaTagsMutation.isPending ? updateMediaTagsMutation.variables?.id : undefined
            }
          />
        ) : (
          <Suspense fallback={<ViewLoadingState />}>
            {activeView === "inverse-index" ? (
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
                  mergeIdentitiesMutation.mutateAsync({ kind, sourceIds, targetId })
                }
                onRefresh={() => inverseIndexQuery.refetch()}
                onRenameIdentity={(kind, id, label) =>
                  renameIdentityMutation.mutateAsync({ id, kind, label })
                }
                refreshing={inverseIndexQuery.isFetching}
                renameError={renameIdentityMutation.error}
                renameErrorIdentity={
                  renameIdentityMutation.isError && renameIdentityMutation.variables
                    ? {
                        id: renameIdentityMutation.variables.id,
                        kind: renameIdentityMutation.variables.kind,
                      }
                    : null
                }
                renamingIdentity={
                  renameIdentityMutation.isPending && renameIdentityMutation.variables
                    ? {
                        id: renameIdentityMutation.variables.id,
                        kind: renameIdentityMutation.variables.kind,
                      }
                    : null
                }
              />
            ) : activeView === "configure" ? (
              <SourceConfigurationPage
                config={sourceConfigQuery.data ?? null}
                error={sourceConfigQuery.error}
                indexError={indexMutation.error}
                indexPending={indexMutation.isPending || indexActive}
                lastIndex={lastIndex}
                loading={sourceConfigQuery.isLoading}
                modelActionPending={
                  downloadModelMutation.isPending || enableModelMutation.isPending
                    ? (
                        (downloadModelMutation.variables ?? enableModelMutation.variables) as
                          | { role: string }
                          | undefined
                      )?.role
                    : undefined
                }
                modelError={downloadModelMutation.error ?? enableModelMutation.error}
                models={modelsQuery.data ?? null}
                modelsError={modelsQuery.error}
                modelsLoading={modelsQuery.isLoading}
                onDownloadModel={(role, model) => downloadModelMutation.mutate({ role, model })}
                onEnableModel={(role, model) => enableModelMutation.mutate({ role, model })}
                onIndex={() => indexMutation.mutate()}
                onSave={(sources) => sourceConfigMutation.mutate(sources)}
                saveError={sourceConfigMutation.error}
                savePending={sourceConfigMutation.isPending}
                saveSuccess={sourceConfigMutation.isSuccess}
              />
            ) : (
              <IndexingConfigurationPage
                config={sourceConfigQuery.data ?? null}
                error={sourceConfigQuery.error}
                indexError={indexMutation.error}
                indexPending={indexMutation.isPending || indexActive}
                lastIndex={lastIndex}
                loading={sourceConfigQuery.isLoading}
                onIndex={() => indexMutation.mutate()}
                onSave={(indexing) => indexingConfigMutation.mutate(indexing)}
                saveError={indexingConfigMutation.error}
                savePending={indexingConfigMutation.isPending}
                saveSuccess={indexingConfigMutation.isSuccess}
              />
            )}
          </Suspense>
        )}
      </div>
    </main>
  );
}

function ViewLoadingState() {
  return (
    <section className="flex min-h-72 items-center justify-center rounded-lg border border-neutral-300 bg-white text-sm font-medium text-neutral-600 shadow-sm">
      <Loader2 className="mr-2 size-4 animate-spin" aria-hidden="true" />
      Loading view
    </section>
  );
}

function createHistoryId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function applyPersonMutation(
  image: SearchResult["image"],
  mutation: IdentityMutationResponse,
): SearchResult["image"] {
  const sourceIds = new Set(mutation.source_ids);
  const targetLabel = mutation.target_label;
  const nextFaces = image.faces.map((face) => {
    if (
      face.person_id === mutation.target_id ||
      (face.person_id && sourceIds.has(face.person_id))
    ) {
      return {
        ...face,
        person_id: mutation.target_id,
        person_label: targetLabel,
      };
    }
    return face;
  });
  const people = new Map<string, SearchResult["image"]["people"][number]>();
  for (const person of image.people) {
    const nextId = sourceIds.has(person.person_id) ? mutation.target_id : person.person_id;
    const nextPerson = {
      ...person,
      label: nextId === mutation.target_id ? targetLabel : person.label,
      person_id: nextId,
    };
    const existing = people.get(nextId);
    if (existing) {
      people.set(nextId, {
        ...existing,
        confidence: Math.max(existing.confidence, nextPerson.confidence),
        face_count: existing.face_count + nextPerson.face_count,
        media_count: Math.max(existing.media_count, nextPerson.media_count),
      });
    } else {
      people.set(nextId, nextPerson);
    }
  }

  return {
    ...image,
    faces: nextFaces,
    people: [...people.values()],
  };
}

function applySpeakerMutation(
  image: SearchResult["image"],
  mutation: IdentityMutationResponse,
): SearchResult["image"] {
  if (!image.audio_analysis) {
    return image;
  }
  const sourceIds = new Set(mutation.source_ids);
  const targetLabel = mutation.target_label ?? mutation.target_id;
  const voiceWeights = new Map<string, number>();
  const recognizedVoices = new Map<
    string,
    NonNullable<SearchResult["image"]["audio_analysis"]>["recognized_voices"][number]
  >();

  for (const voice of image.audio_analysis.recognized_voices) {
    const nextId = sourceIds.has(voice.id) ? mutation.target_id : voice.id;
    const nextVoice = {
      ...voice,
      id: nextId,
      label: nextId === mutation.target_id ? targetLabel : voice.label,
    };
    const existing = recognizedVoices.get(nextId);
    if (!existing) {
      recognizedVoices.set(nextId, nextVoice);
      voiceWeights.set(nextId, Math.max(voice.segment_count, 1));
      continue;
    }

    const existingWeight = voiceWeights.get(nextId) ?? 1;
    const nextWeight = Math.max(voice.segment_count, 1);
    recognizedVoices.set(nextId, {
      ...existing,
      confidence:
        (existing.confidence * existingWeight + voice.confidence * nextWeight) /
        (existingWeight + nextWeight),
      segment_count: existing.segment_count + voice.segment_count,
      total_seconds: roundMillis(existing.total_seconds + voice.total_seconds),
    });
    voiceWeights.set(nextId, existingWeight + nextWeight);
  }

  return {
    ...image,
    audio_analysis: {
      ...image.audio_analysis,
      audio_segments: image.audio_analysis.audio_segments.map((segment) => {
        if (
          segment.speaker_id === mutation.target_id ||
          (segment.speaker_id && sourceIds.has(segment.speaker_id))
        ) {
          return {
            ...segment,
            speaker_id: mutation.target_id,
            speaker_label: targetLabel,
          };
        }
        return segment;
      }),
      recognized_voices: [...recognizedVoices.values()],
    },
  };
}

function roundMillis(value: number) {
  return Math.round(value * 1000) / 1000;
}

import { Button, Input, Label } from "@moritzbrantner/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircle,
  CheckCircle2,
  Database,
  FileAudio,
  FileText,
  History,
  ImageIcon,
  Loader2,
  Search,
  Settings,
  SlidersHorizontal,
  Upload,
  Users,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";
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
  searchMedia,
  startIndexJob,
  updateIndexedMediaTags,
  updateIndexingConfig,
  updateSourceConfig,
} from "./api";
import { MetadataFiltersPanel, ResultSortSelect } from "./components/filter-fields";
import { IndexingConfigurationPage } from "./components/indexing-configuration-page";
import { InverseIndexPage } from "./components/inverse-index-page";
import { JobsPanel } from "./components/jobs-panel";
import { ResultsGrid } from "./components/results-grid";
import { SceneResultsList } from "./components/scene-results-list";
import { SourceConfigurationPage } from "./components/source-configuration-page";
import { StatusMessage } from "./components/status-message";
import {
  formatHistoryTime,
  jobIsActive,
  jobIsTerminal,
  numberFromMetadata,
  sortJobs,
} from "./jobs/job-utils";
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
import type { IndexResponse, SearchResult } from "./types";

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
  });

  const inverseIndexQuery = useQuery({
    queryKey: ["inverse-index"],
    queryFn: fetchInverseIndex,
    enabled: activeView === "inverse-index",
  });

  const modelsQuery = useQuery({
    queryKey: ["models"],
    queryFn: fetchModels,
  });

  const jobsQuery = useQuery({
    queryKey: ["jobs"],
    queryFn: fetchJobs,
    refetchInterval: 2000,
  });

  const jobs = useMemo(() => sortJobs(jobsQuery.data ?? []), [jobsQuery.data]);
  const selectedJob = jobs.find((job) => job.spec.id === selectedJobId) ?? jobs[0] ?? null;
  const latestIndexJob = jobs.find((job) => job.spec.kind?.startsWith("index."));

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
    },
  });

  const enableModelMutation = useMutation({
    mutationFn: ({ model, role }: { model?: string | null; role: string }) =>
      enableModel(role, model),
    onSuccess: (job) => {
      setSelectedJobId(job.spec.id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["models"] });
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
        <header className="flex flex-col gap-4 border-b border-neutral-300 pb-5 lg:flex-row lg:items-start lg:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-sm font-medium text-emerald-700">
              {healthQuery.isLoading ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : healthQuery.isError ? (
                <AlertCircle className="size-4" aria-hidden="true" />
              ) : (
                <CheckCircle2 className="size-4" aria-hidden="true" />
              )}
              <span>{healthQuery.data?.status?.toUpperCase() ?? "STATUS"}</span>
            </div>
            <h1 className="mt-2 text-3xl font-semibold leading-tight tracking-normal text-neutral-950">
              Image Similarity Service
            </h1>
            <p className="mt-2 max-w-4xl truncate text-sm text-neutral-600" title={sourcesLabel}>
              Sources: {sourcesLabel}
            </p>
          </div>

          <div className="flex flex-col gap-2 sm:flex-row lg:items-center">
            <div className="flex min-h-10 flex-wrap rounded-md border border-neutral-300 bg-white p-1 shadow-sm">
              <Button
                aria-label="Open query page"
                aria-pressed={activeView === "search"}
                className={`inline-flex items-center justify-center gap-2 rounded px-3 text-sm font-semibold transition ${
                  activeView === "search"
                    ? "bg-neutral-900 text-white"
                    : "text-neutral-700 hover:bg-neutral-100"
                }`}
                onClick={() => setActiveView("search")}
                type="button"
              >
                <Search className="size-4" aria-hidden="true" />
                <span>Search</span>
              </Button>
              <Button
                aria-label="Open inverse index"
                aria-pressed={activeView === "inverse-index"}
                className={`inline-flex items-center justify-center gap-2 rounded px-3 text-sm font-semibold transition ${
                  activeView === "inverse-index"
                    ? "bg-neutral-900 text-white"
                    : "text-neutral-700 hover:bg-neutral-100"
                }`}
                onClick={() => setActiveView("inverse-index")}
                type="button"
              >
                <Users className="size-4" aria-hidden="true" />
                <span>Registry</span>
              </Button>
              <Button
                aria-label="Open media configuration"
                aria-pressed={activeView === "configure"}
                className={`inline-flex items-center justify-center gap-2 rounded px-3 text-sm font-semibold transition ${
                  activeView === "configure"
                    ? "bg-neutral-900 text-white"
                    : "text-neutral-700 hover:bg-neutral-100"
                }`}
                onClick={() => setActiveView("configure")}
                type="button"
              >
                <Settings className="size-4" aria-hidden="true" />
                <span>Sources</span>
              </Button>
              <Button
                aria-label="Open indexing configuration"
                aria-pressed={activeView === "indexing"}
                className={`inline-flex items-center justify-center gap-2 rounded px-3 text-sm font-semibold transition ${
                  activeView === "indexing"
                    ? "bg-neutral-900 text-white"
                    : "text-neutral-700 hover:bg-neutral-100"
                }`}
                onClick={() => setActiveView("indexing")}
                type="button"
              >
                <SlidersHorizontal className="size-4" aria-hidden="true" />
                <span>Indexing</span>
              </Button>
            </div>
            <Button
              variant="outline"
              className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-400 bg-white px-4 text-sm font-semibold text-neutral-900 shadow-sm transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
              disabled={indexMutation.isPending || indexActive}
              onClick={() => indexMutation.mutate()}
              type="button"
            >
              {indexMutation.isPending || indexActive ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Database className="size-4" aria-hidden="true" />
              )}
              <span>Index Sources</span>
            </Button>
          </div>
        </header>

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
          <>
            <section className="grid gap-5 lg:grid-cols-[360px_minmax(0,1fr)]">
              <form
                className="flex flex-col gap-4 rounded-lg border border-neutral-300 bg-white p-4 shadow-sm"
                onSubmit={handleSubmit}
              >
                <div>
                  <Label className="text-sm font-semibold text-neutral-900" htmlFor="query-image">
                    Query media
                  </Label>
                  <Label
                    className="mt-2 flex min-h-32 cursor-pointer flex-col items-center justify-center gap-2 rounded-md border border-dashed border-neutral-400 bg-neutral-50 px-4 py-5 text-center transition hover:border-emerald-600 hover:bg-emerald-50"
                    htmlFor="query-image"
                  >
                    <Upload className="size-6 text-neutral-600" aria-hidden="true" />
                    <span className="max-w-full truncate text-sm font-medium text-neutral-800">
                      {file?.name ?? "Choose an image, video, audio, or PDF"}
                    </span>
                    <span className="text-xs text-neutral-500">
                      PNG, JPEG, GIF, WebP, BMP, TIFF, MP4, MOV, WebM, MKV, AVI, MP3, WAV, FLAC,
                      M4A, AAC, OGG, Opus, or PDF
                    </span>
                  </Label>
                  <Input
                    accept="image/*,video/*,audio/*,application/pdf,.pdf"
                    className="sr-only"
                    id="query-image"
                    onChange={(event) => handleFileChange(event.target.files?.[0] ?? null)}
                    type="file"
                  />
                </div>

                <div>
                  <Label className="text-sm font-semibold text-neutral-900" htmlFor="limit">
                    Result limit
                  </Label>
                  <Input
                    className="mt-2 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
                    id="limit"
                    max={100}
                    min={1}
                    onChange={(event) => handleLimitChange(event.target.value)}
                    type="number"
                    value={limit}
                  />
                </div>

                <div>
                  <Label
                    className="text-sm font-semibold text-neutral-900"
                    htmlFor="ocr-text-query"
                  >
                    Text in media
                  </Label>
                  <div className="mt-2 flex h-10 items-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 transition focus-within:border-emerald-700 focus-within:ring-2 focus-within:ring-emerald-200">
                    <FileText className="size-4 shrink-0 text-neutral-500" aria-hidden="true" />
                    <Input
                      className="min-w-0 flex-1 bg-transparent outline-none"
                      id="ocr-text-query"
                      onChange={(event) => setOcrTextQuery(event.target.value)}
                      placeholder="Invoice, title, sign"
                      type="search"
                      value={ocrTextQuery}
                    />
                  </div>
                </div>

                <div className="flex gap-2">
                  <Button
                    className="inline-flex h-10 flex-1 items-center justify-center gap-2 rounded-md bg-emerald-700 px-4 text-sm font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={!file || searchMutation.isPending}
                    type="submit"
                  >
                    {searchMutation.isPending ? (
                      <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                    ) : (
                      <Search className="size-4" aria-hidden="true" />
                    )}
                    <span>Search</span>
                  </Button>
                  {file ? (
                    <Button
                      aria-label="Clear selected media"
                      variant="outline"
                      className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-neutral-500 hover:bg-neutral-50"
                      onClick={() => handleFileChange(null)}
                      title="Clear selected media"
                      type="button"
                    >
                      <X className="size-4" aria-hidden="true" />
                    </Button>
                  ) : null}
                </div>

                <StatusMessage
                  indexError={indexMutation.error}
                  lastIndex={lastIndex}
                  searchError={searchMutation.error}
                  searchPending={searchMutation.isPending}
                />
              </form>

              <section className="grid min-h-72 overflow-hidden rounded-lg border border-neutral-300 bg-white shadow-sm">
                {displayedPreviewUrl ? (
                  previewIsVideo ? (
                    <video
                      className="h-full max-h-[420px] w-full bg-black object-contain"
                      controls
                      src={displayedPreviewUrl}
                    />
                  ) : previewIsAudio ? (
                    <div className="flex h-full min-h-72 flex-col items-center justify-center gap-4 bg-neutral-50 p-8">
                      <FileAudio className="size-12 text-neutral-500" aria-hidden="true" />
                      <audio className="w-full max-w-xl" controls src={displayedPreviewUrl} />
                    </div>
                  ) : (
                    <img
                      alt="Query preview"
                      className="h-full max-h-[420px] w-full object-contain"
                      src={displayedPreviewUrl}
                    />
                  )
                ) : (
                  <div className="flex flex-col items-center justify-center gap-3 bg-neutral-50 p-8 text-center text-neutral-500">
                    {previewIsPdf ? (
                      <FileText className="size-12" aria-hidden="true" />
                    ) : (
                      <ImageIcon className="size-12" aria-hidden="true" />
                    )}
                    <span className="text-sm font-medium">
                      {previewIsPdf ? "PDF query selected" : "No query media selected"}
                    </span>
                  </div>
                )}
              </section>
            </section>

            {showMetadataFilters ? (
              <MetadataFiltersPanel
                filters={metadataFilters}
                onChange={handleMetadataFiltersChange}
                sourceTypeOptions={sourceTypeOptions}
              />
            ) : null}

            <section className="grid gap-5 lg:grid-cols-[280px_minmax(0,1fr)]">
              <SearchHistoryList
                activeSearchId={activeSearchId}
                history={searchHistory}
                onSelect={handleHistorySelect}
              />

              <div className="flex min-w-0 flex-col gap-3">
                <div className="flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between">
                  <div>
                    <h2 className="text-lg font-semibold text-neutral-950">Results</h2>
                    <p className="text-sm text-neutral-600">
                      {activeResponse?.scenes.length
                        ? `${activeResponse.scenes.length} scene(s), ${results.length} unique result(s)`
                        : activeResponse
                          ? `${results.length} of ${activeResponse.count} result(s), query pHash ${activeResponse.query_phash}`
                          : searchMutation.isPending
                            ? "Searching indexed media."
                            : "Search results will appear here."}
                    </p>
                  </div>
                  {healthQuery.data ? (
                    <span
                      className="truncate text-sm text-neutral-600"
                      title={healthQuery.data.collection}
                    >
                      Collection: {healthQuery.data.collection}
                    </span>
                  ) : null}
                  <ResultSortSelect onChange={handleResultSortModeChange} value={resultSortMode} />
                </div>

                {activeResponse?.scenes.length ? (
                  <SceneResultsList
                    deletingId={
                      deleteMediaMutation.isPending
                        ? (deleteMediaMutation.variables as string | undefined)
                        : undefined
                    }
                    filters={metadataFilters}
                    onDelete={(id) => deleteMediaMutation.mutate(id)}
                    onUpdateTags={(id, tags) => updateMediaTagsMutation.mutate({ id, tags })}
                    onSelectScene={setSelectedQuerySceneIndex}
                    scenes={activeResponse.scenes}
                    selectedSceneIndex={selectedQuerySceneIndex}
                    resultLimit={activeSearch?.limit ?? limit}
                    sortMode={resultSortMode}
                    tagSavingId={
                      updateMediaTagsMutation.isPending
                        ? updateMediaTagsMutation.variables?.id
                        : undefined
                    }
                  />
                ) : (
                  <ResultsGrid
                    pending={searchMutation.isPending}
                    results={results}
                    searched={Boolean(activeResponse)}
                    deletingId={
                      deleteMediaMutation.isPending
                        ? (deleteMediaMutation.variables as string | undefined)
                        : undefined
                    }
                    onDelete={(id) => deleteMediaMutation.mutate(id)}
                    onUpdateTags={(id, tags) => updateMediaTagsMutation.mutate({ id, tags })}
                    tagSavingId={
                      updateMediaTagsMutation.isPending
                        ? updateMediaTagsMutation.variables?.id
                        : undefined
                    }
                  />
                )}
              </div>
            </section>
          </>
        ) : activeView === "inverse-index" ? (
          <InverseIndexPage
            data={inverseIndexQuery.data ?? null}
            error={inverseIndexQuery.error}
            loading={inverseIndexQuery.isLoading}
            onRefresh={() => inverseIndexQuery.refetch()}
            refreshing={inverseIndexQuery.isFetching}
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
      </div>
    </main>
  );
}

function createHistoryId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function SearchHistoryList({
  activeSearchId,
  history,
  onSelect,
}: {
  activeSearchId: string | null;
  history: SearchHistoryItem[];
  onSelect: (item: SearchHistoryItem) => void;
}) {
  return (
    <aside className="h-fit rounded-lg border border-neutral-300 bg-white p-3 shadow-sm">
      <div className="flex items-center gap-2 px-1 pb-3 text-sm font-semibold text-neutral-950">
        <History className="size-4 text-neutral-600" aria-hidden="true" />
        <span>Search History</span>
      </div>

      {history.length === 0 ? (
        <div className="rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-3 py-5 text-center text-sm text-neutral-500">
          No searches yet.
        </div>
      ) : (
        <ol className="flex flex-col gap-2">
          {history.map((item) => (
            <li key={item.id}>
              <Button
                aria-pressed={item.id === activeSearchId}
                variant={item.id === activeSearchId ? "default" : "outline"}
                className={`flex w-full min-w-0 flex-col gap-1 rounded-md border px-3 py-2 text-left transition ${
                  item.id === activeSearchId
                    ? "border-emerald-700 bg-emerald-50 text-emerald-950"
                    : "border-neutral-200 bg-white text-neutral-900 hover:border-neutral-400 hover:bg-neutral-50"
                }`}
                onClick={() => onSelect(item)}
                title={`${item.fileName}, ${item.response.count} result(s)`}
                type="button"
              >
                <span className="truncate text-sm font-semibold">{item.fileName}</span>
                <span className="flex items-center justify-between gap-2 text-xs text-neutral-600">
                  <span>{formatHistoryTime(item.searchedAt)}</span>
                  <span>
                    {item.response.scenes?.length
                      ? `${item.response.scenes.length} scene(s)`
                      : `${item.response.count} result(s)`}
                  </span>
                </span>
                <span className="text-xs text-neutral-500">Limit {item.limit}</span>
                {item.ocrTextQuery ? (
                  <span className="truncate text-xs text-neutral-500">
                    Text: {item.ocrTextQuery}
                  </span>
                ) : null}
              </Button>
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}

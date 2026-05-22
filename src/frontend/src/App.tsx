import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircle,
  ArrowUpDown,
  Camera,
  CheckCircle2,
  Cloud,
  Database,
  FileAudio,
  FileImage,
  FileText,
  FileVideo,
  Film,
  FolderPlus,
  HardDrive,
  History,
  ImageIcon,
  Info,
  Loader2,
  Plus,
  RotateCw,
  Save,
  Search,
  Settings,
  SlidersHorizontal,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  cancelJob,
  fetchHealth,
  fetchJobEvents,
  fetchJobs,
  fetchSourceConfig,
  searchMedia,
  startIndexJob,
  updateSourceConfig,
} from "./api";
import type {
  IndexResponse,
  JobEvent,
  JobSnapshot,
  PersonSummary,
  SearchResponse,
  SearchResult,
  SearchSceneResponse,
  SourceConfigResponse,
  SourceConfigSource,
  SupportedSourceType,
} from "./types";

const DEFAULT_LIMIT = 12;
const DEFAULT_RESULT_SORT: ResultSortMode = "phash_distance";
const FILTERED_SEARCH_CANDIDATE_MULTIPLIER = 8;
const MAX_SEARCH_CANDIDATES = 500;
const MAX_SEARCH_HISTORY = 8;
const SEARCH_HISTORY_STORAGE_KEY = "image-similarity-search-history";
const SEARCH_HISTORY_QUERY_KEY = ["search-history"] as const;
const AUDIO_EXTENSIONS = [".mp3", ".wav", ".flac", ".m4a", ".aac", ".ogg", ".opus", ".wma"];

const DEFAULT_METADATA_FILTERS = {
  dateFrom: "",
  dateTo: "",
  maxHeight: "",
  maxSizeMb: "",
  maxWidth: "",
  mediaKind: "all",
  minHeight: "",
  minSizeMb: "",
  minWidth: "",
  nameQuery: "",
  nearDuplicate: "all",
  orientation: "all",
  personId: "",
  sourceType: "all",
} satisfies MetadataFilters;

type MetadataFilters = {
  dateFrom: string;
  dateTo: string;
  maxHeight: string;
  maxSizeMb: string;
  maxWidth: string;
  mediaKind: "all" | "static_image" | "animated_gif" | "video_scene" | "audio";
  minHeight: string;
  minSizeMb: string;
  minWidth: string;
  nameQuery: string;
  nearDuplicate: "all" | "exclude" | "only";
  orientation: "all" | "landscape" | "portrait" | "square";
  personId: string;
  sourceType: string;
};

type ResultSortMode =
  | "filename"
  | "modified_newest"
  | "phash_distance"
  | "size_largest"
  | "vector_score";

type SearchHistoryItem = {
  id: string;
  fileName: string;
  filters: MetadataFilters;
  limit: number;
  ocrTextQuery: string;
  queryImageUrl: string | null;
  queryMediaKind: SearchResponse["query_media_kind"];
  sortMode: ResultSortMode;
  searchedAt: string;
  response: SearchResponse;
};

type SearchVariables = {
  filters: MetadataFilters;
  ocrTextQuery: string;
  queryFile: File;
  queryImageUrl: string | null;
  resultLimit: number;
  sortMode: ResultSortMode;
};

type AppView = "configure" | "search";

type SourceDraft = {
  id: string;
  kind: string;
  spec: string;
};

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

  const sourceConfigMutation = useMutation({
    mutationFn: updateSourceConfig,
    onSuccess: (response) => {
      queryClient.setQueryData(["source-config"], response);
      queryClient.invalidateQueries({ queryKey: ["health"] });
    },
  });

  const searchMutation = useMutation({
    mutationFn: ({ filters, ocrTextQuery, queryFile, resultLimit }: SearchVariables) =>
      searchMedia(
        queryFile,
        searchCandidateLimit(resultLimit, filters),
        ocrTextQuery,
        filters.personId,
      ),
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
    if (!file) {
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
      file.type.startsWith("video/") || isAudioFile(file)
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
            <div className="inline-flex h-10 rounded-md border border-neutral-300 bg-white p-1 shadow-sm">
              <button
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
              </button>
              <button
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
              </button>
            </div>
            <button
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
            </button>
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
                  <label className="text-sm font-semibold text-neutral-900" htmlFor="query-image">
                    Query media
                  </label>
                  <label
                    className="mt-2 flex min-h-32 cursor-pointer flex-col items-center justify-center gap-2 rounded-md border border-dashed border-neutral-400 bg-neutral-50 px-4 py-5 text-center transition hover:border-emerald-600 hover:bg-emerald-50"
                    htmlFor="query-image"
                  >
                    <Upload className="size-6 text-neutral-600" aria-hidden="true" />
                    <span className="max-w-full truncate text-sm font-medium text-neutral-800">
                      {file?.name ?? "Choose an image, video, or audio"}
                    </span>
                    <span className="text-xs text-neutral-500">
                      PNG, JPEG, GIF, WebP, BMP, TIFF, MP4, MOV, WebM, MKV, AVI, MP3, WAV, FLAC,
                      M4A, AAC, OGG, or Opus
                    </span>
                  </label>
                  <input
                    accept="image/*,video/*,audio/*"
                    className="sr-only"
                    id="query-image"
                    onChange={(event) => handleFileChange(event.target.files?.[0] ?? null)}
                    type="file"
                  />
                </div>

                <div>
                  <label className="text-sm font-semibold text-neutral-900" htmlFor="limit">
                    Result limit
                  </label>
                  <input
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
                  <label
                    className="text-sm font-semibold text-neutral-900"
                    htmlFor="ocr-text-query"
                  >
                    Text in media
                  </label>
                  <div className="mt-2 flex h-10 items-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 transition focus-within:border-emerald-700 focus-within:ring-2 focus-within:ring-emerald-200">
                    <FileText className="size-4 shrink-0 text-neutral-500" aria-hidden="true" />
                    <input
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
                  <button
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
                  </button>
                  {file ? (
                    <button
                      aria-label="Clear selected media"
                      className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-neutral-500 hover:bg-neutral-50"
                      onClick={() => handleFileChange(null)}
                      title="Clear selected media"
                      type="button"
                    >
                      <X className="size-4" aria-hidden="true" />
                    </button>
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
                    <ImageIcon className="size-12" aria-hidden="true" />
                    <span className="text-sm font-medium">No query media selected</span>
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
                    filters={metadataFilters}
                    onSelectScene={setSelectedQuerySceneIndex}
                    scenes={activeResponse.scenes}
                    selectedSceneIndex={selectedQuerySceneIndex}
                    resultLimit={activeSearch?.limit ?? limit}
                    sortMode={resultSortMode}
                  />
                ) : (
                  <ResultsGrid
                    pending={searchMutation.isPending}
                    results={results}
                    searched={Boolean(activeResponse)}
                  />
                )}
              </div>
            </section>
          </>
        ) : (
          <SourceConfigurationPage
            config={sourceConfigQuery.data ?? null}
            error={sourceConfigQuery.error}
            indexError={indexMutation.error}
            indexPending={indexMutation.isPending || indexActive}
            lastIndex={lastIndex}
            loading={sourceConfigQuery.isLoading}
            onIndex={() => indexMutation.mutate()}
            onSave={(sources) => sourceConfigMutation.mutate(sources)}
            saveError={sourceConfigMutation.error}
            savePending={sourceConfigMutation.isPending}
            saveSuccess={sourceConfigMutation.isSuccess}
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

function JobsPanel({
  cancelPendingJobId,
  error,
  events,
  jobs,
  onCancel,
  onSelectJob,
  selectedJobId,
}: {
  cancelPendingJobId: string | null;
  error: Error | null;
  events: JobEvent[];
  jobs: JobSnapshot[];
  onCancel: (jobId: string) => void;
  onSelectJob: (jobId: string) => void;
  selectedJobId: string | null;
}) {
  const selectedJob = jobs.find((job) => job.spec.id === selectedJobId) ?? jobs[0] ?? null;
  const recentEvents = events.slice(-5).reverse();

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Database className="size-4 text-neutral-600" aria-hidden="true" />
            <h2 className="text-sm font-semibold text-neutral-950">Background Jobs</h2>
          </div>
          <p className="mt-1 text-sm text-neutral-600">
            {selectedJob
              ? `${selectedJob.spec.name} · ${selectedJob.status}`
              : "No background jobs yet."}
          </p>
        </div>
        {selectedJob && jobIsActive(selectedJob) ? (
          <button
            className="inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md border border-red-200 bg-white px-3 text-sm font-semibold text-red-700 transition hover:bg-red-50 disabled:cursor-wait disabled:opacity-60"
            disabled={cancelPendingJobId === selectedJob.spec.id}
            onClick={() => onCancel(selectedJob.spec.id)}
            type="button"
          >
            {cancelPendingJobId === selectedJob.spec.id ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <X className="size-4" aria-hidden="true" />
            )}
            <span>Cancel</span>
          </button>
        ) : null}
      </div>

      {selectedJob ? (
        <div className="mt-4 grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.9fr)]">
          <div className="min-w-0">
            <div className="flex items-center justify-between gap-3 text-sm">
              <span className={`font-semibold ${jobStatusClass(selectedJob.status)}`}>
                {selectedJob.status}
              </span>
              <span className="text-neutral-600">{formatJobTime(selectedJob)}</span>
            </div>
            <JobProgressBar progress={selectedJob.progress} />
            <div className="mt-3 flex flex-wrap gap-2">
              {jobs.slice(0, 6).map((job) => (
                <button
                  className={`max-w-full rounded-md border px-2 py-1 text-left text-xs transition ${
                    job.spec.id === selectedJob.spec.id
                      ? "border-neutral-900 bg-neutral-900 text-white"
                      : "border-neutral-300 bg-neutral-50 text-neutral-700 hover:border-neutral-500"
                  }`}
                  key={job.spec.id}
                  onClick={() => onSelectJob(job.spec.id)}
                  title={job.spec.id}
                  type="button"
                >
                  <span className="block max-w-44 truncate font-semibold">{job.spec.name}</span>
                  <span className="block">{job.status}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="min-w-0">
            <h3 className="text-xs font-semibold uppercase tracking-normal text-neutral-500">
              Recent Events
            </h3>
            <ol className="mt-2 grid max-h-40 gap-2 overflow-auto pr-1">
              {recentEvents.length > 0 ? (
                recentEvents.map((event) => (
                  <li className="rounded-md bg-neutral-50 px-3 py-2 text-sm" key={event.sequence}>
                    <div className="flex items-start justify-between gap-3">
                      <span className="min-w-0 text-neutral-800">{jobEventText(event)}</span>
                      <span className="shrink-0 text-xs text-neutral-500">
                        {formatHistoryTime(event.timestamp)}
                      </span>
                    </div>
                  </li>
                ))
              ) : (
                <li className="rounded-md bg-neutral-50 px-3 py-2 text-sm text-neutral-500">
                  No events recorded.
                </li>
              )}
            </ol>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function JobProgressBar({ progress }: { progress: JobSnapshot["progress"] }) {
  const percent =
    progress?.total && progress.total > 0
      ? Math.min(100, Math.round((progress.completed / progress.total) * 100))
      : null;
  const value = progress
    ? `${progress.completed}${progress.total ? `/${progress.total}` : ""} ${progress.unit}`
    : "Waiting";

  return (
    <div className="mt-2">
      <div className="flex items-center justify-between gap-3 text-xs text-neutral-600">
        <span className="min-w-0 truncate">{progress?.message ?? value}</span>
        <span className="shrink-0">{percent === null ? value : `${percent}%`}</span>
      </div>
      <div className="mt-2 h-2 overflow-hidden rounded bg-neutral-200">
        <div
          className="h-full bg-emerald-700 transition-all"
          style={{ width: `${percent ?? (progress ? 45 : 0)}%` }}
        />
      </div>
    </div>
  );
}

function SourceConfigurationPage({
  config,
  error,
  indexError,
  indexPending,
  lastIndex,
  loading,
  onIndex,
  onSave,
  saveError,
  savePending,
  saveSuccess,
}: {
  config: SourceConfigResponse | null;
  error: Error | null;
  indexError: Error | null;
  indexPending: boolean;
  lastIndex: IndexResponse | null;
  loading: boolean;
  onIndex: () => void;
  onSave: (sources: string[]) => void;
  saveError: Error | null;
  savePending: boolean;
  saveSuccess: boolean;
}) {
  const [drafts, setDrafts] = useState<SourceDraft[]>([]);

  useEffect(() => {
    if (!config) {
      return;
    }

    setDrafts(
      config.sources.map((source) => ({
        id: createHistoryId(),
        kind: source.kind,
        spec: source.spec,
      })),
    );
  }, [config]);

  function updateDraft(id: string, patch: Partial<SourceDraft>) {
    setDrafts((current) =>
      current.map((source) => (source.id === id ? { ...source, ...patch } : source)),
    );
  }

  function addSource(kind = "local") {
    const type =
      config?.supported_source_types.find((item) => item.kind === kind && item.implemented) ??
      config?.supported_source_types.find((item) => item.implemented);
    if (!type) {
      return;
    }
    setDrafts((current) => [
      ...current,
      {
        id: createHistoryId(),
        kind: type.kind,
        spec: type?.example.split(" or ")[0] ?? "",
      },
    ]);
  }

  function removeSource(id: string) {
    setDrafts((current) => current.filter((source) => source.id !== id));
  }

  function saveSources() {
    onSave(drafts.map((source) => source.spec.trim()).filter(Boolean));
  }

  const configuredSources = drafts.map((source) => source.spec.trim()).filter(Boolean);
  const canSave = configuredSources.length > 0 && !savePending;

  if (loading) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading source configuration" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  if (!config) {
    return null;
  }

  return (
    <section className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
      <div className="flex min-w-0 flex-col gap-5">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="flex flex-col gap-3 border-b border-neutral-200 pb-4 sm:flex-row sm:items-start sm:justify-between">
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-neutral-950">Media Sources</h2>
              <p
                className="mt-1 truncate text-sm text-neutral-600"
                title={config.media_sources_file}
              >
                Stored in {config.media_sources_file}
              </p>
            </div>
            <button
              className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
              onClick={() => addSource()}
              type="button"
            >
              <Plus className="size-4" aria-hidden="true" />
              <span>Add Source</span>
            </button>
          </div>

          <div className="mt-4 flex flex-col gap-3">
            {drafts.map((source, index) => (
              <SourceDraftRow
                index={index}
                key={source.id}
                onRemove={() => removeSource(source.id)}
                onUpdate={(patch) => updateDraft(source.id, patch)}
                source={source}
                supportedTypes={config.supported_source_types}
              />
            ))}
            {drafts.length === 0 ? (
              <div className="rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-4 py-8 text-center text-sm text-neutral-500">
                No media sources configured.
              </div>
            ) : null}
          </div>

          <div className="mt-4 flex flex-col gap-3 border-t border-neutral-200 pt-4 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-h-11">
              {savePending ? (
                <Message
                  icon={<Loader2 className="size-4 animate-spin" />}
                  text="Saving source configuration."
                  tone="info"
                />
              ) : saveError ? (
                <Message
                  icon={<AlertCircle className="size-4" />}
                  text={saveError.message}
                  tone="error"
                />
              ) : saveSuccess ? (
                <Message
                  icon={<CheckCircle2 className="size-4" />}
                  text="Saved source configuration."
                  tone="ok"
                />
              ) : (
                <Message
                  icon={<Info className="size-4" />}
                  text="Index sources after changing the source list."
                  tone="info"
                />
              )}
            </div>
            <div className="flex gap-2">
              <button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
                disabled={indexPending}
                onClick={onIndex}
                type="button"
              >
                {indexPending ? (
                  <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Database className="size-4" aria-hidden="true" />
                )}
                <span>Index Sources</span>
              </button>
              <button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-emerald-700 px-4 text-sm font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={!canSave}
                onClick={saveSources}
                type="button"
              >
                {savePending ? (
                  <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Save className="size-4" aria-hidden="true" />
                )}
                <span>Save</span>
              </button>
            </div>
          </div>
          {indexError || lastIndex ? (
            <div className="mt-3">
              {indexError ? (
                <Message
                  icon={<AlertCircle className="size-4" />}
                  text={indexError.message}
                  tone="error"
                />
              ) : lastIndex ? (
                <Message
                  icon={<CheckCircle2 className="size-4" />}
                  text={`Indexed ${lastIndex.indexed} media item(s), skipped ${lastIndex.skipped}, pruned ${lastIndex.pruned}, failed ${lastIndex.failed}.`}
                  tone={lastIndex.failed > 0 ? "warn" : "ok"}
                />
              ) : null}
            </div>
          ) : null}
        </section>

        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="mb-3 flex items-center gap-2 text-sm font-semibold text-neutral-950">
            <FolderPlus className="size-4 text-neutral-600" aria-hidden="true" />
            <span>Configured Source Status</span>
          </div>
          <div className="grid gap-3 md:grid-cols-2">
            {config.sources.map((source) => (
              <SourceStatusCard key={`${source.kind}-${source.spec}`} source={source} />
            ))}
          </div>
        </section>
      </div>

      <aside className="flex h-fit flex-col gap-5">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h2 className="text-sm font-semibold text-neutral-950">Source Types</h2>
          <div className="mt-3 grid gap-2">
            {config.supported_source_types.map((sourceType) => (
              <SupportedSourceTypeRow key={sourceType.kind} sourceType={sourceType} />
            ))}
          </div>
        </section>

        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h2 className="text-sm font-semibold text-neutral-950">Indexing Behavior</h2>
          <dl className="mt-3 grid gap-2 text-sm">
            <Metric label="Collection" value={config.indexing.collection} />
            <Metric label="Images" value={config.indexing.image_extensions.join(", ")} />
            <Metric label="Video" value={config.indexing.video_extensions.join(", ")} />
            <Metric label="Audio" value={config.indexing.audio_extensions.join(", ")} />
            <Metric
              label="Visual embeddings"
              value={
                config.indexing.visual_embedding_enabled
                  ? `${config.indexing.visual_embedding_model} (${config.indexing.visual_embedding_vector_size})`
                  : "disabled"
              }
            />
            <Metric
              label="Faces"
              value={config.indexing.face_analysis_enabled ? "enabled" : "disabled"}
            />
            <Metric
              label="Face confidence"
              value={(config.indexing.face_detection_min_confidence ?? 0).toFixed(2)}
            />
            <Metric
              label="Face threshold"
              value={(config.indexing.face_cluster_threshold ?? 0).toFixed(2)}
            />
            <Metric label="GIF samples" value={config.indexing.gif_sample_frames} />
            <Metric label="GIF motion" value={config.indexing.gif_motion_weight.toFixed(2)} />
            <Metric label="Video stride" value={config.indexing.video_frame_stride} />
            <Metric label="Video cap" value={config.indexing.video_max_frames ?? "none"} />
            <Metric label="OCR" value={config.indexing.ocr_enabled ? "enabled" : "disabled"} />
            <Metric label="OCR frames" value={config.indexing.ocr_max_frames} />
            <Metric
              label="Transcription"
              value={`backend-only (${config.indexing.audio_transcription_enabled ? "enabled" : "disabled"})`}
            />
          </dl>
        </section>
      </aside>
    </section>
  );
}

function SourceDraftRow({
  index,
  onRemove,
  onUpdate,
  source,
  supportedTypes,
}: {
  index: number;
  onRemove: () => void;
  onUpdate: (patch: Partial<SourceDraft>) => void;
  source: SourceDraft;
  supportedTypes: SupportedSourceType[];
}) {
  const inputId = `source-spec-${source.id}`;
  const selectId = `source-kind-${source.id}`;
  const selectedSourceType = supportedTypes.find((sourceType) => sourceType.kind === source.kind);
  const plannedReadOnly = selectedSourceType ? !selectedSourceType.implemented : false;
  const hasKnownType = source.kind === "custom" || selectedSourceType !== undefined;

  return (
    <div className="grid gap-3 rounded-md border border-neutral-200 bg-neutral-50 p-3 md:grid-cols-[180px_minmax(0,1fr)_40px]">
      <div>
        <label className="text-xs font-semibold text-neutral-700" htmlFor={selectId}>
          Source {index + 1}
        </label>
        <select
          className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 disabled:cursor-not-allowed disabled:bg-neutral-100 disabled:text-neutral-500"
          disabled={plannedReadOnly}
          id={selectId}
          onChange={(event) => onUpdate({ kind: event.target.value })}
          value={source.kind}
        >
          {supportedTypes.map((sourceType) => (
            <option
              disabled={!sourceType.implemented}
              key={sourceType.kind}
              value={sourceType.kind}
            >
              {sourceType.label}
              {sourceType.implemented ? "" : " (planned)"}
            </option>
          ))}
          {!hasKnownType ? <option value={source.kind}>{source.kind}</option> : null}
          <option value="custom">Custom</option>
        </select>
      </div>
      <div className="min-w-0">
        <label className="text-xs font-semibold text-neutral-700" htmlFor={inputId}>
          Source spec
        </label>
        <input
          className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 read-only:cursor-not-allowed read-only:bg-neutral-100 read-only:text-neutral-500"
          id={inputId}
          onChange={(event) => onUpdate({ spec: event.target.value })}
          placeholder="/images or minio://bucket/prefix"
          readOnly={plannedReadOnly}
          value={source.spec}
        />
      </div>
      <div className="flex items-end">
        <button
          aria-label={`Remove source ${index + 1}`}
          className="inline-flex h-10 w-10 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-red-300 hover:bg-red-50 hover:text-red-700"
          onClick={onRemove}
          title="Remove source"
          type="button"
        >
          <Trash2 className="size-4" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}

function SourceStatusCard({ source }: { source: SourceConfigSource }) {
  const Icon = sourceKindIcon(source.kind);
  const toneClass =
    {
      not_implemented: "border-amber-200 bg-amber-50 text-amber-900",
      ready: "border-emerald-200 bg-emerald-50 text-emerald-900",
      unavailable: "border-red-200 bg-red-50 text-red-900",
      unsupported: "border-red-200 bg-red-50 text-red-900",
    }[source.status] ?? "border-neutral-200 bg-neutral-50 text-neutral-800";

  return (
    <article className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-neutral-600" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold text-neutral-950" title={source.spec}>
            {source.spec}
          </h3>
          <div className="mt-2 flex flex-wrap gap-2">
            <span className="inline-flex rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
              {source.kind}
            </span>
            <span
              className={`inline-flex rounded-md border px-2 py-1 text-xs font-semibold ${toneClass}`}
            >
              {source.status.replaceAll("_", " ")}
            </span>
          </div>
          {source.detail ? <p className="mt-2 text-xs text-neutral-600">{source.detail}</p> : null}
        </div>
      </div>
    </article>
  );
}

function SupportedSourceTypeRow({ sourceType }: { sourceType: SupportedSourceType }) {
  const Icon = sourceKindIcon(sourceType.kind);

  return (
    <div className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-neutral-600" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-sm font-semibold text-neutral-950">{sourceType.label}</h3>
            <span
              className={`shrink-0 rounded-md border px-2 py-1 text-xs font-semibold ${
                sourceType.implemented
                  ? "border-emerald-200 bg-emerald-50 text-emerald-800"
                  : "border-amber-200 bg-amber-50 text-amber-800"
              }`}
            >
              {sourceType.implemented ? "available" : "planned"}
            </span>
          </div>
          <p className="mt-1 truncate text-xs text-neutral-600" title={sourceType.example}>
            {sourceType.example}
          </p>
        </div>
      </div>
    </div>
  );
}

function sourceKindIcon(kind: string) {
  switch (kind) {
    case "camera":
      return Camera;
    case "minio":
      return Cloud;
    case "video":
      return Film;
    case "local":
      return HardDrive;
    default:
      return FolderPlus;
  }
}

function MetadataFiltersPanel({
  filters,
  onChange,
  sourceTypeOptions,
}: {
  filters: MetadataFilters;
  onChange: (filters: MetadataFilters) => void;
  sourceTypeOptions: string[];
}) {
  function updateFilter<Key extends keyof MetadataFilters>(key: Key, value: MetadataFilters[Key]) {
    onChange({ ...filters, [key]: value });
  }

  const activeFilterCount = countActiveFilters(filters);

  return (
    <fieldset className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <legend className="flex w-full items-center justify-between gap-2 px-1 text-sm font-semibold text-neutral-900">
        <span className="flex items-center gap-2">
          <SlidersHorizontal className="size-4 text-neutral-600" aria-hidden="true" />
          <span>Metadata filters</span>
        </span>
        {activeFilterCount > 0 ? (
          <button
            className="text-xs font-semibold text-emerald-800 transition hover:text-emerald-950"
            onClick={() => onChange(DEFAULT_METADATA_FILTERS)}
            type="button"
          >
            Clear {activeFilterCount}
          </button>
        ) : null}
      </legend>

      <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <div>
          <label className="text-xs font-semibold text-neutral-700" htmlFor="name-query">
            Name or path
          </label>
          <input
            className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
            id="name-query"
            onChange={(event) => updateFilter("nameQuery", event.target.value)}
            placeholder="Filename or folder"
            type="search"
            value={filters.nameQuery}
          />
        </div>

        <div>
          <label className="text-xs font-semibold text-neutral-700" htmlFor="source-type">
            Source type
          </label>
          <select
            className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
            id="source-type"
            onChange={(event) => updateFilter("sourceType", event.target.value)}
            value={filters.sourceType}
          >
            <option value="all">All sources</option>
            {sourceTypeOptions.map((sourceType) => (
              <option key={sourceType} value={sourceType}>
                {sourceType}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="text-xs font-semibold text-neutral-700" htmlFor="person-id">
            Person ID
          </label>
          <input
            className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
            id="person-id"
            onChange={(event) => updateFilter("personId", event.target.value)}
            placeholder="person-..."
            type="search"
            value={filters.personId}
          />
        </div>

        <div>
          <label className="text-xs font-semibold text-neutral-700" htmlFor="media-kind">
            Media type
          </label>
          <select
            className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
            id="media-kind"
            onChange={(event) =>
              updateFilter("mediaKind", event.target.value as MetadataFilters["mediaKind"])
            }
            value={filters.mediaKind}
          >
            <option value="all">All media</option>
            <option value="static_image">Images only</option>
            <option value="animated_gif">GIFs only</option>
            <option value="video_scene">Video scenes only</option>
            <option value="audio">Audio only</option>
          </select>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="near-duplicate">
              Duplicate status
            </label>
            <select
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="near-duplicate"
              onChange={(event) =>
                updateFilter(
                  "nearDuplicate",
                  event.target.value as MetadataFilters["nearDuplicate"],
                )
              }
              value={filters.nearDuplicate}
            >
              <option value="all">All matches</option>
              <option value="only">Near duplicates only</option>
              <option value="exclude">Exclude near duplicates</option>
            </select>
          </div>

          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="orientation">
              Orientation
            </label>
            <select
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="orientation"
              onChange={(event) =>
                updateFilter("orientation", event.target.value as MetadataFilters["orientation"])
              }
              value={filters.orientation}
            >
              <option value="all">Any orientation</option>
              <option value="landscape">Landscape</option>
              <option value="portrait">Portrait</option>
              <option value="square">Square</option>
            </select>
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="date-from">
              Modified after
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="date-from"
              onChange={(event) => updateFilter("dateFrom", event.target.value)}
              type="date"
              value={filters.dateFrom}
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="date-to">
              Modified before
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="date-to"
              onChange={(event) => updateFilter("dateTo", event.target.value)}
              type="date"
              value={filters.dateTo}
            />
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="min-size">
              Min file size (MB)
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="min-size"
              min={0}
              onChange={(event) => updateFilter("minSizeMb", event.target.value)}
              placeholder="Any"
              step="0.1"
              type="number"
              value={filters.minSizeMb}
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="max-size">
              Max file size (MB)
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="max-size"
              min={0}
              onChange={(event) => updateFilter("maxSizeMb", event.target.value)}
              placeholder="Any"
              step="0.1"
              type="number"
              value={filters.maxSizeMb}
            />
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="min-width">
              Minimum width
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="min-width"
              min={0}
              onChange={(event) => updateFilter("minWidth", event.target.value)}
              placeholder="Any"
              type="number"
              value={filters.minWidth}
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="min-height">
              Minimum height
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="min-height"
              min={0}
              onChange={(event) => updateFilter("minHeight", event.target.value)}
              placeholder="Any"
              type="number"
              value={filters.minHeight}
            />
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="max-width">
              Maximum width
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="max-width"
              min={0}
              onChange={(event) => updateFilter("maxWidth", event.target.value)}
              placeholder="Any"
              type="number"
              value={filters.maxWidth}
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-neutral-700" htmlFor="max-height">
              Maximum height
            </label>
            <input
              className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
              id="max-height"
              min={0}
              onChange={(event) => updateFilter("maxHeight", event.target.value)}
              placeholder="Any"
              type="number"
              value={filters.maxHeight}
            />
          </div>
        </div>
      </div>
    </fieldset>
  );
}

function ResultSortSelect({
  onChange,
  value,
}: {
  onChange: (sortMode: ResultSortMode) => void;
  value: ResultSortMode;
}) {
  return (
    <label className="flex w-full items-center gap-2 sm:w-auto">
      <span className="flex shrink-0 items-center gap-2 text-sm font-semibold text-neutral-800">
        <ArrowUpDown className="size-4 text-neutral-600" aria-hidden="true" />
        Sort
      </span>
      <select
        className="h-9 min-w-48 flex-1 rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 sm:flex-none"
        onChange={(event) => onChange(event.target.value as ResultSortMode)}
        value={value}
      >
        <option value="phash_distance">pHash distance</option>
        <option value="vector_score">Visual score</option>
        <option value="modified_newest">Newest modified</option>
        <option value="size_largest">Largest file</option>
        <option value="filename">Filename</option>
      </select>
    </label>
  );
}

function filterResults(results: SearchResult[], filters: MetadataFilters) {
  const nameQuery = filters.nameQuery.trim().toLocaleLowerCase();
  const personId = filters.personId.trim();
  const minSizeBytes = megabytesToBytes(positiveNumber(filters.minSizeMb));
  const maxSizeBytes = megabytesToBytes(positiveNumber(filters.maxSizeMb));
  const minWidth = positiveNumber(filters.minWidth);
  const minHeight = positiveNumber(filters.minHeight);
  const maxWidth = positiveNumber(filters.maxWidth);
  const maxHeight = positiveNumber(filters.maxHeight);
  const modifiedFrom = dateBoundary(filters.dateFrom, "start");
  const modifiedTo = dateBoundary(filters.dateTo, "end");

  return results.filter((result) => {
    const image = result.image;

    if (nameQuery && !imageMatchesNameQuery(image, nameQuery)) {
      return false;
    }

    if (filters.sourceType !== "all" && image.source_type !== filters.sourceType) {
      return false;
    }

    if (filters.mediaKind !== "all" && image.media_kind !== filters.mediaKind) {
      return false;
    }

    if (personId && !(image.people ?? []).some((person) => person.person_id === personId)) {
      return false;
    }

    if (filters.nearDuplicate === "only" && !result.near_duplicate) {
      return false;
    }

    if (filters.nearDuplicate === "exclude" && result.near_duplicate) {
      return false;
    }

    if (
      filters.orientation !== "all" &&
      imageOrientation(image.width, image.height) !== filters.orientation
    ) {
      return false;
    }

    if (minWidth !== null && image.width < minWidth) {
      return false;
    }

    if (minHeight !== null && image.height < minHeight) {
      return false;
    }

    if (maxWidth !== null && image.width > maxWidth) {
      return false;
    }

    if (maxHeight !== null && image.height > maxHeight) {
      return false;
    }

    if (minSizeBytes !== null && image.size_bytes < minSizeBytes) {
      return false;
    }

    if (maxSizeBytes !== null && image.size_bytes > maxSizeBytes) {
      return false;
    }

    if (modifiedFrom !== null && image.modified_at * 1000 < modifiedFrom) {
      return false;
    }

    if (modifiedTo !== null && image.modified_at * 1000 > modifiedTo) {
      return false;
    }

    return true;
  });
}

function sortResults(results: SearchResult[], sortMode: ResultSortMode) {
  return results
    .map((result, index) => ({ index, result }))
    .sort((left, right) => {
      const comparison = compareResults(left.result, right.result, sortMode);
      return comparison === 0 ? left.index - right.index : comparison;
    })
    .map(({ result }) => result);
}

function compareResults(left: SearchResult, right: SearchResult, sortMode: ResultSortMode) {
  switch (sortMode) {
    case "filename":
      return compareFilenames(left, right);
    case "modified_newest":
      return compareDescending(left.image.modified_at, right.image.modified_at, left, right);
    case "size_largest":
      return compareDescending(left.image.size_bytes, right.image.size_bytes, left, right);
    case "vector_score":
      return compareDescending(left.vector_score, right.vector_score, left, right);
    case "phash_distance":
      return compareHashDistance(left, right);
  }
}

function compareHashDistance(left: SearchResult, right: SearchResult) {
  const leftDistance = left.hash_distance;
  const rightDistance = right.hash_distance;

  if (leftDistance === null && rightDistance === null) {
    return compareDescending(left.vector_score, right.vector_score, left, right);
  }

  if (leftDistance === null) {
    return 1;
  }

  if (rightDistance === null) {
    return -1;
  }

  return (
    leftDistance - rightDistance ||
    compareDescending(left.vector_score, right.vector_score, left, right)
  );
}

function compareDescending(
  leftValue: number,
  rightValue: number,
  leftResult: SearchResult,
  rightResult: SearchResult,
) {
  return rightValue - leftValue || compareHashDistanceForTie(leftResult, rightResult);
}

function compareHashDistanceForTie(left: SearchResult, right: SearchResult) {
  const leftDistance = left.hash_distance;
  const rightDistance = right.hash_distance;

  if (leftDistance === null && rightDistance === null) {
    return compareFilenames(left, right);
  }

  if (leftDistance === null) {
    return 1;
  }

  if (rightDistance === null) {
    return -1;
  }

  return leftDistance - rightDistance || compareFilenames(left, right);
}

function compareFilenames(left: SearchResult, right: SearchResult) {
  return (
    left.image.filename.localeCompare(right.image.filename, undefined, {
      sensitivity: "base",
    }) ||
    left.image.relative_path.localeCompare(right.image.relative_path, undefined, {
      sensitivity: "base",
    })
  );
}

function sourceTypesFor(results: SearchResult[], currentSourceType: string) {
  const sourceTypes = new Set(results.map((result) => result.image.source_type).filter(Boolean));
  if (currentSourceType !== "all") {
    sourceTypes.add(currentSourceType);
  }

  return Array.from(sourceTypes).sort((left, right) => left.localeCompare(right));
}

function positiveNumber(value: string) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function megabytesToBytes(value: number | null) {
  return value === null ? null : value * 1024 * 1024;
}

function dateBoundary(value: string, boundary: "end" | "start") {
  if (!value) {
    return null;
  }

  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return null;
  }

  if (boundary === "end") {
    date.setDate(date.getDate() + 1);
    date.setMilliseconds(date.getMilliseconds() - 1);
  }

  return date.getTime();
}

function imageMatchesNameQuery(image: SearchResult["image"], nameQuery: string) {
  return [image.filename, image.relative_path, image.path, image.source_uri ?? ""].some((value) =>
    value.toLocaleLowerCase().includes(nameQuery),
  );
}

function imageOrientation(width: number, height: number): MetadataFilters["orientation"] {
  if (width === height) {
    return "square";
  }

  return width > height ? "landscape" : "portrait";
}

function isAudioFile(file: File) {
  if (file.type.startsWith("audio/")) {
    return true;
  }

  const lowerName = file.name.toLocaleLowerCase();
  return AUDIO_EXTENSIONS.some((extension) => lowerName.endsWith(extension));
}

async function createQueryPreview(file: File) {
  if (file.type === "image/gif" || file.name.toLowerCase().endsWith(".gif")) {
    return null;
  }

  try {
    const image = await createImageBitmap(file);
    const maxSize = 640;
    const scale = Math.min(1, maxSize / Math.max(image.width, image.height));
    const width = Math.max(1, Math.round(image.width * scale));
    const height = Math.max(1, Math.round(image.height * scale));
    const canvas = document.createElement("canvas");

    canvas.width = width;
    canvas.height = height;
    canvas.getContext("2d")?.drawImage(image, 0, 0, width, height);
    image.close();

    return canvas.toDataURL("image/jpeg", 0.82);
  } catch {
    return null;
  }
}

function loadSearchHistory() {
  if (typeof localStorage === "undefined") {
    return [];
  }

  try {
    const stored = localStorage.getItem(SEARCH_HISTORY_STORAGE_KEY);
    const parsed: unknown = stored ? JSON.parse(stored) : [];

    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed
      .filter(isSearchHistoryItem)
      .map((item) => ({
        ...item,
        filters: normalizeMetadataFilters(item.filters),
        ocrTextQuery: stringFilter(item.ocrTextQuery),
        queryImageUrl: normalizeStoredPreviewUrl(item.queryImageUrl),
        queryMediaKind: item.queryMediaKind ?? item.response.query_media_kind ?? "static_image",
        response: normalizeSearchResponse(item.response),
        sortMode: normalizeResultSortMode(item.sortMode),
      }))
      .slice(0, MAX_SEARCH_HISTORY);
  } catch {
    return [];
  }
}

function saveSearchHistory(history: SearchHistoryItem[]) {
  if (typeof localStorage === "undefined") {
    return;
  }

  try {
    localStorage.setItem(SEARCH_HISTORY_STORAGE_KEY, JSON.stringify(history));
  } catch {
    localStorage.removeItem(SEARCH_HISTORY_STORAGE_KEY);
  }
}

function isSearchHistoryItem(value: unknown): value is SearchHistoryItem {
  if (!value || typeof value !== "object") {
    return false;
  }

  const item = value as Partial<SearchHistoryItem>;
  const response = item.response;
  return (
    typeof item.id === "string" &&
    typeof item.fileName === "string" &&
    (item.filters === undefined || isFilterObject(item.filters)) &&
    typeof item.limit === "number" &&
    (item.ocrTextQuery === undefined || typeof item.ocrTextQuery === "string") &&
    (typeof item.queryImageUrl === "string" ||
      item.queryImageUrl === null ||
      item.queryImageUrl === undefined) &&
    (item.queryMediaKind === undefined ||
      item.queryMediaKind === "static_image" ||
      item.queryMediaKind === "animated_gif" ||
      item.queryMediaKind === "video" ||
      item.queryMediaKind === "audio") &&
    (item.sortMode === undefined || isResultSortMode(item.sortMode)) &&
    typeof item.searchedAt === "string" &&
    Boolean(response) &&
    Array.isArray(response?.results) &&
    typeof response?.count === "number" &&
    typeof response?.query_phash === "string"
  );
}

function isFilterObject(value: unknown) {
  return Boolean(value) && typeof value === "object";
}

function normalizeMetadataFilters(filters: unknown): MetadataFilters {
  if (!filters || typeof filters !== "object") {
    return DEFAULT_METADATA_FILTERS;
  }

  const partial = filters as Partial<MetadataFilters>;
  return {
    ...DEFAULT_METADATA_FILTERS,
    dateFrom: stringFilter(partial.dateFrom),
    dateTo: stringFilter(partial.dateTo),
    maxHeight: stringFilter(partial.maxHeight),
    maxSizeMb: stringFilter(partial.maxSizeMb),
    maxWidth: stringFilter(partial.maxWidth),
    mediaKind: isMediaKindFilter(partial.mediaKind)
      ? partial.mediaKind
      : DEFAULT_METADATA_FILTERS.mediaKind,
    minHeight: stringFilter(partial.minHeight),
    minSizeMb: stringFilter(partial.minSizeMb),
    minWidth: stringFilter(partial.minWidth),
    nameQuery: stringFilter(partial.nameQuery),
    nearDuplicate: isNearDuplicateFilter(partial.nearDuplicate)
      ? partial.nearDuplicate
      : DEFAULT_METADATA_FILTERS.nearDuplicate,
    orientation: isOrientationFilter(partial.orientation)
      ? partial.orientation
      : DEFAULT_METADATA_FILTERS.orientation,
    personId: stringFilter(partial.personId),
    sourceType: stringFilter(partial.sourceType) || DEFAULT_METADATA_FILTERS.sourceType,
  };
}

function stringFilter(value: unknown) {
  return typeof value === "string" ? value : "";
}

function normalizeStoredPreviewUrl(value: unknown) {
  if (typeof value !== "string" || value.startsWith("blob:")) {
    return null;
  }

  return value;
}

function isMediaKindFilter(value: unknown): value is MetadataFilters["mediaKind"] {
  return (
    value === "all" ||
    value === "static_image" ||
    value === "animated_gif" ||
    value === "video_scene" ||
    value === "audio"
  );
}

function isNearDuplicateFilter(value: unknown): value is MetadataFilters["nearDuplicate"] {
  return value === "all" || value === "exclude" || value === "only";
}

function isOrientationFilter(value: unknown): value is MetadataFilters["orientation"] {
  return value === "all" || value === "landscape" || value === "portrait" || value === "square";
}

function normalizeSearchResponse(response: SearchHistoryItem["response"]): SearchResponse {
  return {
    ...response,
    results: Array.isArray(response.results) ? response.results.map(normalizeSearchResult) : [],
    query_audio_analysis: response.query_audio_analysis ?? null,
    query_ocr_text: response.query_ocr_text ?? "",
    query_media_kind: response.query_media_kind ?? "static_image",
    scenes: Array.isArray(response.scenes)
      ? response.scenes.map((scene) => ({
          ...scene,
          results: Array.isArray(scene.results) ? scene.results.map(normalizeSearchResult) : [],
        }))
      : [],
  };
}

function normalizeSearchResult(result: SearchResult): SearchResult {
  return {
    ...result,
    image: {
      ...result.image,
      faces: Array.isArray(result.image.faces) ? result.image.faces : [],
      people: Array.isArray(result.image.people) ? result.image.people : [],
      visual_embedding_model: result.image.visual_embedding_model ?? null,
    },
  };
}

function normalizeResultSortMode(value: unknown): ResultSortMode {
  return isResultSortMode(value) ? value : DEFAULT_RESULT_SORT;
}

function isResultSortMode(value: unknown): value is ResultSortMode {
  return (
    value === "filename" ||
    value === "modified_newest" ||
    value === "phash_distance" ||
    value === "size_largest" ||
    value === "vector_score"
  );
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
              <button
                aria-pressed={item.id === activeSearchId}
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
              </button>
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}

function formatHistoryTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

function sortJobs(jobs: JobSnapshot[]) {
  return [...jobs].sort(
    (left, right) => new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
  );
}

function jobIsActive(job: JobSnapshot) {
  return job.status === "Queued" || job.status === "Running" || job.status === "Cancelling";
}

function jobIsTerminal(status: JobSnapshot["status"]) {
  return status === "Succeeded" || status === "Failed" || status === "Cancelled";
}

function numberFromMetadata(value: string | undefined) {
  if (value === undefined) {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function jobStatusClass(status: JobSnapshot["status"]) {
  return {
    Cancelled: "text-amber-700",
    Cancelling: "text-amber-700",
    Failed: "text-red-700",
    Queued: "text-neutral-700",
    Running: "text-emerald-700",
    Succeeded: "text-emerald-700",
  }[status];
}

function formatJobTime(job: JobSnapshot) {
  const value = job.finished_at ?? job.started_at ?? job.created_at;
  return formatHistoryTime(value);
}

function jobEventText(event: JobEvent) {
  const kind = event.kind;
  if ("StatusChanged" in kind) {
    return kind.StatusChanged.message
      ? `${kind.StatusChanged.status}: ${kind.StatusChanged.message}`
      : kind.StatusChanged.status;
  }
  if ("Progress" in kind) {
    const progress = kind.Progress;
    const total = progress.total ? `/${progress.total}` : "";
    return progress.message ?? `${progress.completed}${total} ${progress.unit}`;
  }
  if ("Log" in kind) {
    return kind.Log.message;
  }
  if ("Metadata" in kind) {
    return `${kind.Metadata.key}: ${kind.Metadata.value}`;
  }
  return "Artifact recorded";
}

function StatusMessage({
  indexError,
  lastIndex,
  searchError,
  searchPending,
}: {
  indexError: Error | null;
  lastIndex: IndexResponse | null;
  searchError: Error | null;
  searchPending: boolean;
}) {
  if (searchPending) {
    return (
      <Message
        icon={<Loader2 className="size-4 animate-spin" />}
        text="Searching indexed media."
        tone="info"
      />
    );
  }

  if (searchError) {
    return (
      <Message icon={<AlertCircle className="size-4" />} text={searchError.message} tone="error" />
    );
  }

  if (indexError) {
    return (
      <Message icon={<AlertCircle className="size-4" />} text={indexError.message} tone="error" />
    );
  }

  if (lastIndex) {
    const tone = lastIndex.failed > 0 ? "warn" : "ok";
    const text = `Indexed ${lastIndex.indexed} media item(s), skipped ${lastIndex.skipped}, pruned ${lastIndex.pruned}, failed ${lastIndex.failed}.`;
    return <Message icon={<CheckCircle2 className="size-4" />} text={text} tone={tone} />;
  }

  return (
    <Message
      icon={<RotateCw className="size-4" />}
      text="Index sources before searching fresh data."
      tone="info"
    />
  );
}

function Message({
  icon,
  text,
  tone,
}: {
  icon: React.ReactNode;
  text: string;
  tone: "error" | "info" | "ok" | "warn";
}) {
  const toneClass = {
    error: "border-red-200 bg-red-50 text-red-800",
    info: "border-neutral-200 bg-neutral-50 text-neutral-700",
    ok: "border-emerald-200 bg-emerald-50 text-emerald-800",
    warn: "border-amber-200 bg-amber-50 text-amber-800",
  }[tone];

  return (
    <p
      className={`flex min-h-11 items-start gap-2 rounded-md border px-3 py-2 text-sm ${toneClass}`}
    >
      <span className="mt-0.5 shrink-0" aria-hidden="true">
        {icon}
      </span>
      <span>{text}</span>
    </p>
  );
}

function ResultsGrid({
  pending,
  results,
  searched,
}: {
  pending: boolean;
  results: SearchResult[];
  searched: boolean;
}) {
  if (pending) {
    return (
      <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600">
        <Loader2 className="size-7 animate-spin" aria-label="Loading search results" />
      </div>
    );
  }

  if (!searched) {
    return <EmptyResults text="Choose a query image, video, or audio and run a search." />;
  }

  if (results.length === 0) {
    return <EmptyResults text="No indexed media matched this query." />;
  }

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {results.map((result) => (
        <ResultCard key={result.image.id} result={result} />
      ))}
    </div>
  );
}

function SceneResultsList({
  filters,
  onSelectScene,
  resultLimit,
  scenes,
  selectedSceneIndex,
  sortMode,
}: {
  filters: MetadataFilters;
  onSelectScene: (sceneIndex: number) => void;
  resultLimit: number;
  scenes: SearchSceneResponse[];
  selectedSceneIndex: number | null;
  sortMode: ResultSortMode;
}) {
  const selectedScene =
    scenes.find((scene) => scene.scene_index === selectedSceneIndex) ?? scenes[0];
  const selectedResults = selectedScene
    ? sortResults(filterResults(selectedScene.results, filters), sortMode).slice(0, resultLimit)
    : [];
  const isAudioBits = scenes.some((scene) => scene.scene_kind === "audio_bit");
  const segmentLabel = isAudioBits ? "Bit" : "Scene";
  const SegmentIcon = isAudioBits ? FileAudio : FileVideo;

  return (
    <div className="flex flex-col gap-5">
      <div className="rounded-lg border border-neutral-300 bg-white p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2 text-sm font-semibold text-neutral-950">
          <SegmentIcon className="size-4 text-neutral-600" aria-hidden="true" />
          <span>Query segment</span>
        </div>
        <div className="flex gap-2 overflow-x-auto pb-1">
          {scenes.map((scene) => (
            <button
              aria-pressed={scene.scene_index === selectedScene?.scene_index}
              className={`inline-flex h-10 shrink-0 items-center justify-center rounded-md border px-3 text-sm font-semibold transition ${
                scene.scene_index === selectedScene?.scene_index
                  ? "border-emerald-700 bg-emerald-50 text-emerald-950"
                  : "border-neutral-300 bg-white text-neutral-800 hover:border-neutral-500 hover:bg-neutral-50"
              }`}
              key={scene.scene_index}
              onClick={() => onSelectScene(scene.scene_index)}
              type="button"
            >
              {segmentLabel} {scene.scene_index + 1} · {formatSeconds(scene.start_seconds)}-
              {formatSeconds(scene.end_seconds)}
              {scene.speaker_label ? ` · ${scene.speaker_label}` : ""}
            </button>
          ))}
        </div>
      </div>

      {selectedScene ? (
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="mb-3 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <h3 className="text-sm font-semibold text-neutral-950">
                {segmentLabel} {selectedScene.scene_index + 1}
              </h3>
              <p className="text-xs text-neutral-600">
                {formatSeconds(selectedScene.start_seconds)}-
                {formatSeconds(selectedScene.end_seconds)}
                {isAudioBits
                  ? selectedScene.speaker_label
                    ? ` · ${selectedScene.speaker_label}`
                    : ""
                  : ` · frames ${selectedScene.start_frame}-${selectedScene.end_frame}`}
              </p>
            </div>
            {selectedScene.clip_url ? (
              <a
                className="inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
                href={selectedScene.clip_url}
                rel="noreferrer"
                target="_blank"
              >
                <FileVideo className="size-4" aria-hidden="true" />
                <span>Open query clip</span>
              </a>
            ) : null}
          </div>
          <ResultsGrid pending={false} results={selectedResults} searched />
        </section>
      ) : null}
    </div>
  );
}

function VideoSceneLinks({ image }: { image: SearchResult["image"] }) {
  if (image.media_kind !== "video_scene") {
    return null;
  }

  const start = image.scene_start_seconds ?? 0;
  const end = image.scene_end_seconds ?? start;
  const fullVideoUrl = image.full_video_url
    ? `${image.full_video_url}#t=${start.toFixed(3)},${end.toFixed(3)}`
    : null;

  return (
    <div className="grid gap-2">
      <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
        {formatSeconds(start)}-{formatSeconds(end)}
        {image.scene_start_frame !== null && image.scene_end_frame !== null
          ? ` · frames ${image.scene_start_frame}-${image.scene_end_frame}`
          : ""}
      </div>
      <div className="flex flex-wrap gap-2">
        {fullVideoUrl ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={fullVideoUrl}
            rel="noreferrer"
            target="_blank"
          >
            <FileVideo className="size-3.5" aria-hidden="true" />
            <span>Full video</span>
          </a>
        ) : null}
        {image.scene_clip_url ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={image.scene_clip_url}
            rel="noreferrer"
            target="_blank"
          >
            <FileVideo className="size-3.5" aria-hidden="true" />
            <span>Scene clip</span>
          </a>
        ) : null}
      </div>
    </div>
  );
}

function AudioLinks({ image }: { image: SearchResult["image"] }) {
  if (image.media_kind !== "audio" || !image.full_audio_url) {
    return null;
  }

  const start = image.scene_start_seconds ?? null;
  const end = image.scene_end_seconds ?? null;
  const audioUrl =
    start !== null && end !== null
      ? `${image.full_audio_url}#t=${start.toFixed(3)},${end.toFixed(3)}`
      : image.full_audio_url;

  return (
    <div className="grid gap-2">
      <audio className="w-full" controls src={audioUrl} />
      {start !== null && end !== null ? (
        <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
          {formatSeconds(start)}-{formatSeconds(end)}
        </div>
      ) : null}
      <a
        className="inline-flex h-8 w-fit items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
        href={audioUrl}
        rel="noreferrer"
        target="_blank"
      >
        <FileAudio className="size-3.5" aria-hidden="true" />
        <span>Open audio</span>
      </a>
    </div>
  );
}

function EmptyResults({ text }: { text: string }) {
  return (
    <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white p-8 text-center text-sm text-neutral-500">
      <div className="flex flex-col items-center gap-3">
        <FileImage className="size-8" aria-hidden="true" />
        <span>{text}</span>
      </div>
    </div>
  );
}

function ResultCard({ result }: { result: SearchResult }) {
  const image = result.image;
  const faces = image.faces ?? [];
  const people = image.people ?? [];
  const previewUrl = image.animated_thumbnail_url ?? image.thumbnail_url;

  return (
    <article className="overflow-hidden rounded-lg border border-neutral-300 bg-white shadow-sm">
      <div className="grid aspect-[4/3] place-items-center bg-neutral-200">
        {previewUrl ? (
          <img alt="" className="h-full w-full object-contain" loading="lazy" src={previewUrl} />
        ) : (
          <ImageIcon className="size-9 text-neutral-500" aria-hidden="true" />
        )}
      </div>
      <div className="flex flex-col gap-3 p-4">
        <div className="min-w-0">
          <h3 className="truncate text-sm font-semibold text-neutral-950" title={image.filename}>
            {image.filename}
          </h3>
          <p className="mt-1 truncate text-xs text-neutral-600" title={image.relative_path}>
            {image.relative_path}
          </p>
        </div>

        <dl className="grid gap-2 text-sm">
          <Metric label="Visual score" value={result.vector_score.toFixed(4)} />
          <Metric label="pHash distance" value={result.hash_distance ?? "n/a"} />
          {result.ocr_score !== null && result.ocr_score !== undefined ? (
            <Metric label="OCR score" value={result.ocr_score.toFixed(2)} />
          ) : null}
          <Metric label="Dimensions" value={`${image.width} x ${image.height}`} />
          <Metric label="File size" value={formatFileSize(image.size_bytes)} />
          <Metric label="Modified" value={formatModifiedAt(image.modified_at)} />
          {image.frame_count ? <Metric label="Frames" value={image.frame_count} /> : null}
          {image.duration_ms ? (
            <Metric label="Duration" value={formatDuration(image.duration_ms)} />
          ) : null}
          {image.audio_analysis ? (
            <Metric
              label="Speech"
              value={image.audio_analysis.speech_detected ? "Detected" : "Not detected"}
            />
          ) : null}
          {image.audio_analysis ? (
            <Metric label="Speech ratio" value={formatPercent(image.audio_analysis.speech_ratio)} />
          ) : null}
          {image.audio_analysis?.audio_segments?.length ? (
            <Metric label="Audio bits" value={image.audio_analysis.audio_segments.length} />
          ) : null}
          {image.audio_analysis?.recognized_voices?.length ? (
            <Metric
              label="Voices"
              value={image.audio_analysis.recognized_voices.map((voice) => voice.label).join(", ")}
            />
          ) : null}
          {image.audio_analysis?.transcript_text ? (
            <Metric label="Transcript" value={image.audio_analysis.transcript_text} />
          ) : null}
          {image.audio_analysis?.transcript_language ? (
            <Metric label="Language" value={image.audio_analysis.transcript_language} />
          ) : null}
          {image.audio_analysis?.tempo_bpm ? (
            <Metric label="Tempo" value={`${image.audio_analysis.tempo_bpm.toFixed(1)} BPM`} />
          ) : null}
          {image.audio_analysis?.tempo_bpm ? (
            <Metric
              label="Tempo confidence"
              value={formatPercent(image.audio_analysis.tempo_confidence)}
            />
          ) : null}
          {image.ocr_text ? <Metric label="OCR text" value={image.ocr_text} /> : null}
          {faces.length ? <Metric label="Faces" value={faces.length} /> : null}
          {people.length ? (
            <Metric label="People" value={people.map(personDisplayName).join(", ")} />
          ) : null}
        </dl>

        <VideoSceneLinks image={image} />
        <AudioLinks image={image} />

        <div className="flex flex-wrap gap-2">
          {image.media_kind === "animated_gif" ? (
            <span className="inline-flex w-fit rounded-md border border-sky-300 bg-sky-50 px-2 py-1 text-xs font-semibold text-sky-900">
              GIF
            </span>
          ) : null}
          {image.media_kind === "video_scene" ? (
            <span className="inline-flex w-fit rounded-md border border-violet-300 bg-violet-50 px-2 py-1 text-xs font-semibold text-violet-900">
              Video scene
            </span>
          ) : null}
          {image.media_kind === "audio" ? (
            <span className="inline-flex w-fit rounded-md border border-emerald-300 bg-emerald-50 px-2 py-1 text-xs font-semibold text-emerald-900">
              Audio
            </span>
          ) : null}
          {image.ocr_text ? (
            <span className="inline-flex w-fit rounded-md border border-cyan-300 bg-cyan-50 px-2 py-1 text-xs font-semibold text-cyan-900">
              OCR
            </span>
          ) : null}
          {image.audio_analysis?.speech_detected ? (
            <span className="inline-flex w-fit rounded-md border border-teal-300 bg-teal-50 px-2 py-1 text-xs font-semibold text-teal-900">
              Speech
            </span>
          ) : null}
          {image.audio_analysis?.recognized_voices?.map((voice) => (
            <span
              className="inline-flex w-fit rounded-md border border-lime-300 bg-lime-50 px-2 py-1 text-xs font-semibold text-lime-900"
              key={voice.id}
            >
              {voice.label}
            </span>
          ))}
          {image.audio_analysis?.transcript_text ? (
            <span className="inline-flex w-fit rounded-md border border-fuchsia-300 bg-fuchsia-50 px-2 py-1 text-xs font-semibold text-fuchsia-900">
              Transcript
            </span>
          ) : null}
          {image.audio_analysis?.tempo_bpm ? (
            <span className="inline-flex w-fit rounded-md border border-rose-300 bg-rose-50 px-2 py-1 text-xs font-semibold text-rose-900">
              {image.audio_analysis.tempo_bpm.toFixed(0)} BPM
            </span>
          ) : null}
          {faces.length ? (
            <span className="inline-flex w-fit rounded-md border border-indigo-300 bg-indigo-50 px-2 py-1 text-xs font-semibold text-indigo-900">
              Faces {faces.length}
            </span>
          ) : null}
          {people.map((person) => (
            <span
              className="inline-flex w-fit rounded-md border border-purple-300 bg-purple-50 px-2 py-1 text-xs font-semibold text-purple-900"
              key={person.person_id}
              title={person.person_id}
            >
              {personDisplayName(person)}
            </span>
          ))}
          {result.query_scene_index !== null && result.query_scene_index !== undefined ? (
            <span className="inline-flex w-fit rounded-md border border-neutral-300 bg-neutral-50 px-2 py-1 text-xs font-semibold text-neutral-700">
              Query scene {result.query_scene_index + 1}
            </span>
          ) : null}
          {result.near_duplicate ? (
            <span className="inline-flex w-fit rounded-md border border-amber-300 bg-amber-50 px-2 py-1 text-xs font-semibold text-amber-900">
              Near duplicate
            </span>
          ) : null}
        </div>
      </div>
    </article>
  );
}

function countActiveFilters(filters: MetadataFilters) {
  return Object.entries(filters).filter(([key, value]) => {
    const defaultValue = DEFAULT_METADATA_FILTERS[key as keyof MetadataFilters];
    return value !== defaultValue;
  }).length;
}

function searchCandidateLimit(resultLimit: number, filters: MetadataFilters) {
  if (countActiveFilters(filters) === 0) {
    return resultLimit;
  }

  return Math.min(resultLimit * FILTERED_SEARCH_CANDIDATE_MULTIPLIER, MAX_SEARCH_CANDIDATES);
}

function formatDuration(durationMs: number) {
  return `${(durationMs / 1000).toFixed(1)}s`;
}

function formatSeconds(seconds: number) {
  return `${seconds.toFixed(1)}s`;
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

function personDisplayName(person: PersonSummary) {
  return person.label?.trim() || person.person_id;
}

function formatFileSize(sizeBytes: number) {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }

  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }

  return `${(sizeBytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatModifiedAt(modifiedAt: number) {
  if (!Number.isFinite(modifiedAt) || modifiedAt <= 0) {
    return "n/a";
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).format(new Date(modifiedAt * 1000));
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-neutral-600">{label}</dt>
      <dd className="min-w-0 truncate font-medium text-neutral-900">{value}</dd>
    </div>
  );
}

import {
  Alert,
  AlertDescription,
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Checkbox,
  Input,
  Label,
  NativeSelect,
  Progress,
  Textarea,
} from "@moritzbrantner/ui";
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
  Mic2,
  Plus,
  RotateCw,
  Save,
  Search,
  Settings,
  SlidersHorizontal,
  Trash2,
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
import { formatFileSize, formatModifiedAt, mediaKindLabel } from "./lib/format";
import { isAudioFile, isPdfFile } from "./lib/media";
import type {
  IndexResponse,
  InverseIndexLocation,
  InverseIndexResponse,
  JobEvent,
  JobSnapshot,
  ModelRuntimeStatus,
  ModelsResponse,
  PersonSummary,
  SearchResponse,
  SearchResult,
  SearchSceneResponse,
  SourceConfigResponse,
  SourceConfigSource,
  SourceIndexingConfig,
  SupportedSourceType,
} from "./types";

const DEFAULT_LIMIT = 12;
const DEFAULT_RESULT_SORT: ResultSortMode = "phash_distance";
const MAX_SEARCH_HISTORY = 8;
const SEARCH_HISTORY_STORAGE_KEY = "image-similarity-search-history";
const SEARCH_HISTORY_QUERY_KEY = ["search-history"] as const;

const DEFAULT_METADATA_FILTERS = {
  cameraQuery: "",
  captureDateFrom: "",
  captureDateTo: "",
  dateFrom: "",
  dateTo: "",
  hasGps: "all",
  keywordQuery: "",
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
  cameraQuery: string;
  captureDateFrom: string;
  captureDateTo: string;
  dateFrom: string;
  dateTo: string;
  hasGps: "all" | "yes" | "no";
  keywordQuery: string;
  maxHeight: string;
  maxSizeMb: string;
  maxWidth: string;
  mediaKind:
    | "all"
    | "static_image"
    | "animated_gif"
    | "video_scene"
    | "audio"
    | "pdf_page"
    | "pdf_document";
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
  | "captured_newest"
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

type AppView = "configure" | "indexing" | "inverse-index" | "search";

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

function completeIndexingConfig(indexing: SourceIndexingConfig): SourceIndexingConfig {
  return {
    ...indexing,
    audio_extensions: indexing.audio_extensions ?? [".mp3", ".wav"],
    audio_transcription_enabled: indexing.audio_transcription_enabled ?? false,
    collection: indexing.collection ?? "",
    face_analysis_enabled: indexing.face_analysis_enabled ?? false,
    face_cluster_threshold: indexing.face_cluster_threshold ?? 0.38,
    face_detection_min_confidence: indexing.face_detection_min_confidence ?? 0.75,
    face_max_frames_per_media: indexing.face_max_frames_per_media ?? 8,
    face_min_cluster_images: indexing.face_min_cluster_images ?? 2,
    gif_default_frame_delay_ms: indexing.gif_default_frame_delay_ms ?? 100,
    gif_max_decode_frames: indexing.gif_max_decode_frames ?? 512,
    gif_motion_weight: indexing.gif_motion_weight ?? 0.2,
    gif_preview_frames: indexing.gif_preview_frames ?? 16,
    gif_sample_frames: indexing.gif_sample_frames ?? 16,
    image_extensions: indexing.image_extensions ?? [".jpg", ".jpeg", ".png", ".gif"],
    ocr_enabled: indexing.ocr_enabled ?? false,
    ocr_max_frames: indexing.ocr_max_frames ?? 4,
    pdf_extensions: indexing.pdf_extensions ?? [".pdf"],
    pdf_max_pages: indexing.pdf_max_pages ?? 100,
    pdf_render_dpi: indexing.pdf_render_dpi ?? 144,
    pdf_summary_pages: indexing.pdf_summary_pages ?? 8,
    video_extensions: indexing.video_extensions ?? [".mp4", ".mov"],
    video_frame_stride: indexing.video_frame_stride ?? 30,
    video_max_frames: indexing.video_max_frames ?? null,
    visual_embedding_enabled: indexing.visual_embedding_enabled ?? false,
    visual_embedding_model: indexing.visual_embedding_model ?? "",
    visual_embedding_vector_size: indexing.visual_embedding_vector_size ?? 0,
  };
}

function splitExtensionDraft(value: string) {
  return value
    .split(/[\s,]+/)
    .map((extension) => extension.trim())
    .filter(Boolean);
}

function normalizeExtensionDraft(value: string[]) {
  return Array.from(
    new Set(
      value
        .map((extension) => extension.trim().toLowerCase())
        .filter(Boolean)
        .map((extension) => (extension.startsWith(".") ? extension : `.${extension}`)),
    ),
  );
}

function numberInputValue(value: string, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
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
    <Card className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <CardHeader className="flex flex-col gap-3 p-0 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Database className="size-4 text-neutral-600" aria-hidden="true" />
            <CardTitle
              aria-level={2}
              className="text-sm font-semibold text-neutral-950"
              role="heading"
            >
              Background Jobs
            </CardTitle>
          </div>
          <p className="mt-1 text-sm text-neutral-600">
            {selectedJob
              ? `${selectedJob.spec.name} · ${selectedJob.status}`
              : "No background jobs yet."}
          </p>
        </div>
        {selectedJob && jobIsActive(selectedJob) ? (
          <Button
            variant="outline"
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
          </Button>
        ) : null}
      </CardHeader>

      {selectedJob ? (
        <CardContent className="mt-4 grid gap-4 p-0 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.9fr)]">
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
                <Button
                  variant={job.spec.id === selectedJob.spec.id ? "default" : "outline"}
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
                </Button>
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
        </CardContent>
      ) : null}
    </Card>
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
      <Progress className="mt-2 h-2 bg-neutral-200" value={percent ?? (progress ? 45 : 0)} />
    </div>
  );
}

function InverseIndexPage({
  data,
  error,
  loading,
  onRefresh,
  refreshing,
}: {
  data: InverseIndexResponse | null;
  error: Error | null;
  loading: boolean;
  onRefresh: () => void;
  refreshing: boolean;
}) {
  if (loading && !data) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading inverse index" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  const people = sortPeopleEntries(data?.people ?? []);
  const speakers = sortSpeakerEntries(data?.speakers ?? []);

  return (
    <section className="flex flex-col gap-5">
      <div className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <h2 className="text-lg font-semibold text-neutral-950">Inverse Index</h2>
            <p className="mt-1 text-sm text-neutral-600">
              Key findings grouped by identity with the indexed media locations where they occur.
            </p>
          </div>
          <Button
            variant="outline"
            className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
            disabled={refreshing}
            onClick={onRefresh}
            type="button"
          >
            {refreshing ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <RotateCw className="size-4" aria-hidden="true" />
            )}
            <span>Refresh</span>
          </Button>
        </div>

        <dl className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <RegistryMetric label="Indexed media" value={data?.indexed_media ?? 0} />
          <RegistryMetric label="People" value={people.length} />
          <RegistryMetric label="Speakers" value={speakers.length} />
          <RegistryMetric label="Decode warnings" value={data?.errors.length ?? 0} />
        </dl>
      </div>

      {data?.errors.length ? (
        <Message
          icon={<AlertCircle className="size-4" />}
          text={`${data.errors.length} indexed payload(s) could not be read. The registry shows the remaining media.`}
          tone="warn"
        />
      ) : null}

      <div className="grid gap-5 xl:grid-cols-2">
        <RegistrySection
          emptyText="No indexed people yet. Enable face analysis, index sources, then refresh this page."
          entries={people}
          icon={<Users className="size-4 text-neutral-600" aria-hidden="true" />}
          kind="person"
          title="Depicted People"
        />
        <RegistrySection
          emptyText="No recognized speakers yet. Enable audio analysis, index audio sources, then refresh this page."
          entries={speakers}
          icon={<Mic2 className="size-4 text-neutral-600" aria-hidden="true" />}
          kind="speaker"
          title="Recognized Speakers"
        />
      </div>
    </section>
  );
}

function RegistryMetric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2">
      <dt className="text-xs font-semibold text-neutral-500">{label}</dt>
      <dd className="mt-1 text-2xl font-semibold text-neutral-950">{value}</dd>
    </div>
  );
}

function RegistrySection({
  emptyText,
  entries,
  icon,
  kind,
  title,
}: {
  emptyText: string;
  entries: Array<InverseIndexResponse["people"][number] | InverseIndexResponse["speakers"][number]>;
  icon: React.ReactNode;
  kind: "person" | "speaker";
  title: string;
}) {
  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <div className="flex items-center gap-2">
        {icon}
        <h3 className="text-sm font-semibold text-neutral-950">{title}</h3>
      </div>

      {entries.length === 0 ? (
        <div className="mt-4 rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-4 py-8 text-center text-sm text-neutral-500">
          {emptyText}
        </div>
      ) : (
        <div className="mt-4 grid gap-3">
          {entries.map((entry) => (
            <RegistryEntryCard entry={entry} key={`${kind}-${entry.id}`} kind={kind} />
          ))}
        </div>
      )}
    </section>
  );
}

function RegistryEntryCard({
  entry,
  kind,
}: {
  entry: InverseIndexResponse["people"][number] | InverseIndexResponse["speakers"][number];
  kind: "person" | "speaker";
}) {
  const isSpeaker = kind === "speaker";
  const primaryCount = isSpeaker
    ? `${(entry as InverseIndexResponse["speakers"][number]).segment_count} segment(s)`
    : `${(entry as InverseIndexResponse["people"][number]).face_count} face(s)`;

  return (
    <article className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <h4 className="truncate text-sm font-semibold text-neutral-950" title={entry.id}>
            {entry.label?.trim() || entry.id}
          </h4>
          <p className="mt-1 truncate text-xs text-neutral-600">{entry.id}</p>
        </div>
        <div className="flex shrink-0 flex-wrap gap-2">
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {entry.media_count} media
          </span>
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {primaryCount}
          </span>
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {formatPercent(entry.confidence)}
          </span>
        </div>
      </div>

      {isSpeaker ? (
        <p className="mt-2 text-xs text-neutral-600">
          {formatDurationSeconds((entry as InverseIndexResponse["speakers"][number]).total_seconds)}
        </p>
      ) : null}

      <div className="mt-3 grid max-h-96 gap-2 overflow-auto pr-1">
        {entry.locations.map((location) => (
          <RegistryLocationRow key={`${entry.id}-${location.media_id}`} location={location} />
        ))}
      </div>
    </article>
  );
}

function RegistryLocationRow({ location }: { location: InverseIndexLocation }) {
  const previewUrl = location.thumbnail_url;
  const openUrl = location.scene_clip_url ?? location.media_url;

  return (
    <div className="grid gap-3 rounded-md border border-neutral-200 bg-white p-2 sm:grid-cols-[64px_minmax(0,1fr)_auto]">
      <div className="grid aspect-square place-items-center overflow-hidden rounded bg-neutral-200">
        {previewUrl ? (
          <img alt="" className="h-full w-full object-cover" loading="lazy" src={previewUrl} />
        ) : (
          <ImageIcon className="size-6 text-neutral-500" aria-hidden="true" />
        )}
      </div>
      <div className="min-w-0">
        <h5 className="truncate text-sm font-semibold text-neutral-950" title={location.filename}>
          {location.filename}
        </h5>
        <p className="mt-1 truncate text-xs text-neutral-600" title={location.relative_path}>
          {location.relative_path}
        </p>
        <div className="mt-2 flex flex-wrap gap-2 text-xs">
          <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
            {mediaKindLabel(location.media_kind)}
          </span>
          <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
            {location.occurrence_count} hit(s)
          </span>
          {location.frame_indices.length ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              Frames {compactNumberList(location.frame_indices)}
            </span>
          ) : null}
          {location.start_seconds !== null && location.end_seconds !== null ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              {formatSeconds(location.start_seconds)}-{formatSeconds(location.end_seconds)}
            </span>
          ) : null}
          {location.page_number ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              Page {location.page_number}
            </span>
          ) : null}
        </div>
      </div>
      {openUrl ? (
        <a
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
          href={openUrl}
          rel="noreferrer"
          target="_blank"
        >
          <FileText className="size-4" aria-hidden="true" />
          <span>Open</span>
        </a>
      ) : null}
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
  modelActionPending,
  modelError,
  models,
  modelsError,
  modelsLoading,
  onDownloadModel,
  onEnableModel,
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
  modelActionPending?: string;
  modelError: Error | null;
  models: ModelsResponse | null;
  modelsError: Error | null;
  modelsLoading: boolean;
  onDownloadModel: (role: string, model?: string | null) => void;
  onEnableModel: (role: string, model?: string | null) => void;
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
  const mediaSourcesWritable = config?.media_sources_writable ?? true;
  const canSave = configuredSources.length > 0 && mediaSourcesWritable && !savePending;

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

  const indexing = completeIndexingConfig(config.indexing);

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
              {config.media_sources_seed_file ? (
                <p
                  className="mt-1 truncate text-xs text-neutral-500"
                  title={config.media_sources_seed_file}
                >
                  Seeded from {config.media_sources_seed_file}
                </p>
              ) : null}
            </div>
            <Button
              variant="outline"
              className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
              onClick={() => addSource()}
              type="button"
            >
              <Plus className="size-4" aria-hidden="true" />
              <span>Add Source</span>
            </Button>
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
              ) : !mediaSourcesWritable ? (
                <Message
                  icon={<AlertCircle className="size-4" />}
                  text="Source configuration file is not writable."
                  tone="error"
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
              <Button
                variant="outline"
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
              </Button>
              <Button
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
              </Button>
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
        <ModelStatusPanel
          actionPendingRole={modelActionPending}
          error={modelError ?? modelsError}
          loading={modelsLoading}
          models={models?.models ?? []}
          onDownload={onDownloadModel}
          onEnable={onEnableModel}
        />

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
            <Metric label="Images" value={indexing.image_extensions.join(", ")} />
            <Metric label="Video" value={indexing.video_extensions.join(", ")} />
            <Metric label="Audio" value={indexing.audio_extensions.join(", ")} />
            <Metric label="PDF" value={indexing.pdf_extensions.join(", ")} />
            <Metric
              label="Visual embeddings"
              value={
                indexing.visual_embedding_enabled
                  ? `${indexing.visual_embedding_model} (${indexing.visual_embedding_vector_size})`
                  : "disabled"
              }
            />
            <Metric label="Faces" value={indexing.face_analysis_enabled ? "enabled" : "disabled"} />
            <Metric
              label="Face confidence"
              value={indexing.face_detection_min_confidence.toFixed(2)}
            />
            <Metric label="Face threshold" value={indexing.face_cluster_threshold.toFixed(2)} />
            <Metric label="GIF samples" value={indexing.gif_sample_frames} />
            <Metric label="GIF motion" value={indexing.gif_motion_weight.toFixed(2)} />
            <Metric label="Video stride" value={indexing.video_frame_stride} />
            <Metric label="Video cap" value={indexing.video_max_frames ?? "none"} />
            <Metric label="PDF DPI" value={indexing.pdf_render_dpi} />
            <Metric label="PDF page cap" value={indexing.pdf_max_pages} />
            <Metric label="PDF summary pages" value={indexing.pdf_summary_pages} />
            <Metric label="OCR" value={indexing.ocr_enabled ? "enabled" : "disabled"} />
            <Metric label="OCR frames" value={indexing.ocr_max_frames} />
            <Metric
              label="Transcription"
              value={`backend-only (${indexing.audio_transcription_enabled ? "enabled" : "disabled"})`}
            />
          </dl>
        </section>
      </aside>
    </section>
  );
}

function IndexingConfigurationPage({
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
  onSave: (indexing: SourceIndexingConfig) => void;
  saveError: Error | null;
  savePending: boolean;
  saveSuccess: boolean;
}) {
  const [draft, setDraft] = useState<SourceIndexingConfig | null>(null);

  useEffect(() => {
    if (!config) {
      return;
    }
    setDraft(completeIndexingConfig(config.indexing));
  }, [config]);

  function updateDraft<Key extends keyof SourceIndexingConfig>(
    key: Key,
    value: SourceIndexingConfig[Key],
  ) {
    setDraft((current) => (current ? { ...current, [key]: value } : current));
  }

  function saveDraft() {
    if (!draft) {
      return;
    }
    onSave({
      ...draft,
      audio_extensions: normalizeExtensionDraft(draft.audio_extensions),
      image_extensions: normalizeExtensionDraft(draft.image_extensions),
      pdf_extensions: normalizeExtensionDraft(draft.pdf_extensions),
    });
  }

  if (loading) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading indexing configuration" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  if (!draft) {
    return null;
  }

  const canSave =
    !savePending &&
    normalizeExtensionDraft(draft.image_extensions).length > 0 &&
    normalizeExtensionDraft(draft.audio_extensions).length > 0 &&
    normalizeExtensionDraft(draft.pdf_extensions).length > 0;

  return (
    <section className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
      <div className="flex min-w-0 flex-col gap-5">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="flex flex-col gap-3 border-b border-neutral-200 pb-4 sm:flex-row sm:items-start sm:justify-between">
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-neutral-950">Indexing Configuration</h2>
              <p className="mt-1 text-sm text-neutral-600">
                Applies to future indexing jobs in this running service.
              </p>
            </div>
            <div className="flex gap-2">
              <Button
                variant="outline"
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
              </Button>
              <Button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-emerald-700 px-4 text-sm font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={!canSave}
                onClick={saveDraft}
                type="button"
              >
                {savePending ? (
                  <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Save className="size-4" aria-hidden="true" />
                )}
                <span>Save</span>
              </Button>
            </div>
          </div>

          <div className="mt-4 grid gap-5">
            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <FileImage className="size-4 text-neutral-600" aria-hidden="true" />
                <span>File Types</span>
              </h3>
              <div className="mt-3 grid gap-3 lg:grid-cols-3">
                <ExtensionsField
                  id="image-extensions"
                  label="Image extensions"
                  onChange={(value) => updateDraft("image_extensions", value)}
                  value={draft.image_extensions}
                />
                <ExtensionsField
                  id="audio-extensions"
                  label="Audio extensions"
                  onChange={(value) => updateDraft("audio_extensions", value)}
                  value={draft.audio_extensions}
                />
                <ExtensionsField
                  id="pdf-extensions"
                  label="PDF extensions"
                  onChange={(value) => updateDraft("pdf_extensions", value)}
                  value={draft.pdf_extensions}
                />
              </div>
            </section>

            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <SlidersHorizontal className="size-4 text-neutral-600" aria-hidden="true" />
                <span>Analysis</span>
              </h3>
              <div className="mt-3 grid gap-3 md:grid-cols-3">
                <ToggleField
                  checked={draft.face_analysis_enabled}
                  label="Face analysis"
                  onChange={(value) => updateDraft("face_analysis_enabled", value)}
                />
                <ToggleField
                  checked={draft.ocr_enabled}
                  label="OCR"
                  onChange={(value) => updateDraft("ocr_enabled", value)}
                />
                <ToggleField
                  checked={draft.audio_transcription_enabled}
                  label="Audio transcription"
                  onChange={(value) => updateDraft("audio_transcription_enabled", value)}
                />
              </div>
            </section>

            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <Film className="size-4 text-neutral-600" aria-hidden="true" />
                <span>Video and GIF</span>
              </h3>
              <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                <NumberField
                  id="video-frame-stride"
                  label="Video frame stride"
                  min={1}
                  onChange={(value) => updateDraft("video_frame_stride", value)}
                  value={draft.video_frame_stride}
                />
                <OptionalNumberField
                  id="video-max-frames"
                  label="Video max frames"
                  min={1}
                  onChange={(value) => updateDraft("video_max_frames", value)}
                  value={draft.video_max_frames}
                />
                <NumberField
                  id="gif-sample-frames"
                  label="GIF sample frames"
                  min={1}
                  onChange={(value) => updateDraft("gif_sample_frames", value)}
                  value={draft.gif_sample_frames}
                />
                <NumberField
                  id="gif-max-decode-frames"
                  label="GIF decode cap"
                  min={1}
                  onChange={(value) => updateDraft("gif_max_decode_frames", value)}
                  value={draft.gif_max_decode_frames}
                />
                <NumberField
                  id="gif-preview-frames"
                  label="GIF preview frames"
                  min={1}
                  onChange={(value) => updateDraft("gif_preview_frames", value)}
                  value={draft.gif_preview_frames}
                />
                <NumberField
                  id="gif-default-frame-delay"
                  label="GIF frame delay (ms)"
                  min={1}
                  onChange={(value) => updateDraft("gif_default_frame_delay_ms", value)}
                  value={draft.gif_default_frame_delay_ms}
                />
                <NumberField
                  id="gif-motion-weight"
                  label="GIF motion weight"
                  max={1}
                  min={0}
                  onChange={(value) => updateDraft("gif_motion_weight", value)}
                  step={0.01}
                  value={draft.gif_motion_weight}
                />
              </div>
            </section>

            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <FileText className="size-4 text-neutral-600" aria-hidden="true" />
                <span>PDF, OCR, and Faces</span>
              </h3>
              <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                <NumberField
                  id="pdf-render-dpi"
                  label="PDF render DPI"
                  max={300}
                  min={72}
                  onChange={(value) => updateDraft("pdf_render_dpi", value)}
                  value={draft.pdf_render_dpi}
                />
                <NumberField
                  id="pdf-max-pages"
                  label="PDF page cap"
                  min={1}
                  onChange={(value) => updateDraft("pdf_max_pages", value)}
                  value={draft.pdf_max_pages}
                />
                <NumberField
                  id="pdf-summary-pages"
                  label="PDF summary pages"
                  max={256}
                  min={1}
                  onChange={(value) => updateDraft("pdf_summary_pages", value)}
                  value={draft.pdf_summary_pages}
                />
                <NumberField
                  id="ocr-max-frames"
                  label="OCR frames"
                  max={64}
                  min={1}
                  onChange={(value) => updateDraft("ocr_max_frames", value)}
                  value={draft.ocr_max_frames}
                />
                <NumberField
                  id="face-confidence"
                  label="Face confidence"
                  max={1}
                  min={0}
                  onChange={(value) => updateDraft("face_detection_min_confidence", value)}
                  step={0.01}
                  value={draft.face_detection_min_confidence}
                />
                <NumberField
                  id="face-threshold"
                  label="Face threshold"
                  max={2}
                  min={0}
                  onChange={(value) => updateDraft("face_cluster_threshold", value)}
                  step={0.01}
                  value={draft.face_cluster_threshold}
                />
                <NumberField
                  id="face-min-cluster-images"
                  label="Face cluster images"
                  min={1}
                  onChange={(value) => updateDraft("face_min_cluster_images", value)}
                  value={draft.face_min_cluster_images}
                />
                <NumberField
                  id="face-max-frames"
                  label="Face frames per media"
                  min={1}
                  onChange={(value) => updateDraft("face_max_frames_per_media", value)}
                  value={draft.face_max_frames_per_media}
                />
              </div>
            </section>
          </div>

          <div className="mt-4 border-t border-neutral-200 pt-4">
            {savePending ? (
              <Message
                icon={<Loader2 className="size-4 animate-spin" />}
                text="Saving indexing configuration."
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
                text="Saved indexing configuration."
                tone="ok"
              />
            ) : (
              <Message
                icon={<Info className="size-4" />}
                text="Run indexing after changing analysis settings."
                tone="info"
              />
            )}
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
          </div>
        </section>
      </div>

      <aside className="flex h-fit flex-col gap-5">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h2 className="text-sm font-semibold text-neutral-950">Runtime Bound</h2>
          <dl className="mt-3 grid gap-2 text-sm">
            <Metric label="Collection" value={draft.collection} />
            <Metric label="Video extensions" value={draft.video_extensions.join(", ")} />
            <Metric
              label="Visual embeddings"
              value={
                draft.visual_embedding_enabled
                  ? `${draft.visual_embedding_model} (${draft.visual_embedding_vector_size})`
                  : "disabled"
              }
            />
          </dl>
        </section>
      </aside>
    </section>
  );
}

function ExtensionsField({
  id,
  label,
  onChange,
  value,
}: {
  id: string;
  label: string;
  onChange: (value: string[]) => void;
  value: string[];
}) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Textarea
        className="mt-1 min-h-24 w-full resize-y rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
        id={id}
        onChange={(event) => onChange(splitExtensionDraft(event.target.value))}
        spellCheck={false}
        value={value.join(", ")}
      />
    </div>
  );
}

function ToggleField({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <Label className="flex h-12 items-center justify-between gap-3 rounded-md border border-neutral-200 bg-neutral-50 px-3 text-sm font-semibold text-neutral-800">
      <span>{label}</span>
      <Checkbox
        checked={checked}
        className="size-4 accent-emerald-700"
        onCheckedChange={(value) => onChange(value === true)}
      />
    </Label>
  );
}

function NumberField({
  id,
  label,
  max,
  min,
  onChange,
  step = 1,
  value,
}: {
  id: string;
  label: string;
  max?: number;
  min: number;
  onChange: (value: number) => void;
  step?: number;
  value: number;
}) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Input
        className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
        id={id}
        max={max}
        min={min}
        onChange={(event) => onChange(numberInputValue(event.target.value, min))}
        step={step}
        type="number"
        value={value}
      />
    </div>
  );
}

function OptionalNumberField({
  id,
  label,
  min,
  onChange,
  value,
}: {
  id: string;
  label: string;
  min: number;
  onChange: (value: number | null) => void;
  value: number | null;
}) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Input
        className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
        id={id}
        min={min}
        onChange={(event) =>
          onChange(event.target.value === "" ? null : numberInputValue(event.target.value, min))
        }
        placeholder="No cap"
        type="number"
        value={value ?? ""}
      />
    </div>
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
        <Label className="text-xs font-semibold text-neutral-700" htmlFor={selectId}>
          Source {index + 1}
        </Label>
        <NativeSelect
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
        </NativeSelect>
      </div>
      <div className="min-w-0">
        <Label className="text-xs font-semibold text-neutral-700" htmlFor={inputId}>
          Source spec
        </Label>
        <Input
          className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 read-only:cursor-not-allowed read-only:bg-neutral-100 read-only:text-neutral-500"
          id={inputId}
          onChange={(event) => onUpdate({ spec: event.target.value })}
          placeholder="/images or minio://bucket/prefix"
          readOnly={plannedReadOnly}
          value={source.spec}
        />
      </div>
      <div className="flex items-end">
        <Button
          aria-label={`Remove source ${index + 1}`}
          variant="outline"
          className="inline-flex h-10 w-10 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-red-300 hover:bg-red-50 hover:text-red-700"
          onClick={onRemove}
          title="Remove source"
          type="button"
        >
          <Trash2 className="size-4" aria-hidden="true" />
        </Button>
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

function ModelStatusPanel({
  actionPendingRole,
  error,
  loading,
  models,
  onDownload,
  onEnable,
}: {
  actionPendingRole?: string;
  error: Error | null;
  loading: boolean;
  models: ModelRuntimeStatus[];
  onDownload: (role: string, model?: string | null) => void;
  onEnable: (role: string, model?: string | null) => void;
}) {
  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <h2 className="text-sm font-semibold text-neutral-950">Model Status</h2>
      {loading ? (
        <div className="mt-3 flex items-center gap-2 text-sm text-neutral-600">
          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          <span>Checking models.</span>
        </div>
      ) : error ? (
        <div className="mt-3">
          <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />
        </div>
      ) : (
        <div className="mt-3 grid gap-3">
          {models.map((model) => {
            const pending = actionPendingRole === model.role;
            return (
              <article
                className="rounded-md border border-neutral-200 bg-neutral-50 p-3"
                key={model.role}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="text-sm font-semibold text-neutral-950">{model.label}</h3>
                    <p className="mt-1 truncate text-xs text-neutral-600" title={model.configured}>
                      {model.configured}
                    </p>
                  </div>
                  <span
                    className={`shrink-0 rounded-md border px-2 py-1 text-xs font-semibold ${
                      model.active
                        ? "border-emerald-200 bg-emerald-50 text-emerald-800"
                        : model.cached
                          ? "border-sky-200 bg-sky-50 text-sky-800"
                          : "border-amber-200 bg-amber-50 text-amber-800"
                    }`}
                  >
                    {model.active ? "active" : model.cached ? "cached" : "missing"}
                  </span>
                </div>
                {model.detail ? (
                  <p className="mt-2 text-xs text-neutral-600">{model.detail}</p>
                ) : null}
                <div className="mt-3 flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
                    disabled={pending || model.cached}
                    onClick={() => onDownload(model.role, model.configured)}
                    type="button"
                  >
                    {pending && !model.cached ? (
                      <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <Cloud className="size-3.5" aria-hidden="true" />
                    )}
                    <span>Download</span>
                  </Button>
                  <Button
                    variant="outline"
                    className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={pending || !model.cached}
                    onClick={() => onEnable(model.role, model.configured)}
                    type="button"
                  >
                    {pending && model.cached ? (
                      <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <CheckCircle2 className="size-3.5" aria-hidden="true" />
                    )}
                    <span>Enable</span>
                  </Button>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
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
    case "s3":
      return Cloud;
    case "video":
      return Film;
    case "local":
      return HardDrive;
    default:
      return FolderPlus;
  }
}

type FieldInputProps = React.ComponentProps<typeof Input> & {
  label: string;
};

function FieldInput({ className = "", id, label, ...props }: FieldInputProps) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Input
        className={`mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 ${className}`}
        id={id}
        {...props}
      />
    </div>
  );
}

type FieldSelectProps = React.ComponentProps<typeof NativeSelect> & {
  label: string;
};

function FieldSelect({ children, className = "", id, label, ...props }: FieldSelectProps) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <NativeSelect className={`mt-1 w-full ${className}`} id={id} {...props}>
        {children}
      </NativeSelect>
    </div>
  );
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
          <Button
            variant="ghost"
            className="text-xs font-semibold text-emerald-800 transition hover:text-emerald-950"
            onClick={() => onChange(DEFAULT_METADATA_FILTERS)}
            type="button"
          >
            Clear {activeFilterCount}
          </Button>
        ) : null}
      </legend>

      <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <FieldInput
          id="name-query"
          label="Name or path"
          onChange={(event) => updateFilter("nameQuery", event.target.value)}
          placeholder="Filename or folder"
          type="search"
          value={filters.nameQuery}
        />

        <FieldSelect
          id="source-type"
          label="Source type"
          onChange={(event) => updateFilter("sourceType", event.target.value)}
          value={filters.sourceType}
        >
          <option value="all">All sources</option>
          {sourceTypeOptions.map((sourceType) => (
            <option key={sourceType} value={sourceType}>
              {sourceType}
            </option>
          ))}
        </FieldSelect>

        <FieldInput
          id="person-id"
          label="Person ID"
          onChange={(event) => updateFilter("personId", event.target.value)}
          placeholder="person-..."
          type="search"
          value={filters.personId}
        />

        <FieldSelect
          id="media-kind"
          label="Media type"
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
          <option value="pdf_document">PDF documents only</option>
          <option value="pdf_page">PDF pages only</option>
        </FieldSelect>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldSelect
            id="near-duplicate"
            label="Duplicate status"
            onChange={(event) =>
              updateFilter("nearDuplicate", event.target.value as MetadataFilters["nearDuplicate"])
            }
            value={filters.nearDuplicate}
          >
            <option value="all">All matches</option>
            <option value="only">Near duplicates only</option>
            <option value="exclude">Exclude near duplicates</option>
          </FieldSelect>

          <FieldSelect
            id="orientation"
            label="Orientation"
            onChange={(event) =>
              updateFilter("orientation", event.target.value as MetadataFilters["orientation"])
            }
            value={filters.orientation}
          >
            <option value="all">Any orientation</option>
            <option value="landscape">Landscape</option>
            <option value="portrait">Portrait</option>
            <option value="square">Square</option>
          </FieldSelect>
        </div>

        <FieldInput
          id="camera-query"
          label="Camera/lens"
          onChange={(event) => updateFilter("cameraQuery", event.target.value)}
          placeholder="Make, model, or lens"
          type="search"
          value={filters.cameraQuery}
        />

        <FieldInput
          id="keyword-query"
          label="Keyword"
          onChange={(event) => updateFilter("keywordQuery", event.target.value)}
          placeholder="Tag or subject"
          type="search"
          value={filters.keywordQuery}
        />

        <FieldSelect
          id="has-gps"
          label="GPS metadata"
          onChange={(event) =>
            updateFilter("hasGps", event.target.value as MetadataFilters["hasGps"])
          }
          value={filters.hasGps}
        >
          <option value="all">Any GPS metadata</option>
          <option value="yes">Has GPS</option>
          <option value="no">No GPS</option>
        </FieldSelect>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="date-from"
            label="Modified after"
            onChange={(event) => updateFilter("dateFrom", event.target.value)}
            type="date"
            value={filters.dateFrom}
          />

          <FieldInput
            id="date-to"
            label="Modified before"
            onChange={(event) => updateFilter("dateTo", event.target.value)}
            type="date"
            value={filters.dateTo}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="capture-date-from"
            label="Captured after"
            onChange={(event) => updateFilter("captureDateFrom", event.target.value)}
            type="date"
            value={filters.captureDateFrom}
          />

          <FieldInput
            id="capture-date-to"
            label="Captured before"
            onChange={(event) => updateFilter("captureDateTo", event.target.value)}
            type="date"
            value={filters.captureDateTo}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="min-size"
            label="Min file size (MB)"
            min={0}
            onChange={(event) => updateFilter("minSizeMb", event.target.value)}
            placeholder="Any"
            step="0.1"
            type="number"
            value={filters.minSizeMb}
          />

          <FieldInput
            id="max-size"
            label="Max file size (MB)"
            min={0}
            onChange={(event) => updateFilter("maxSizeMb", event.target.value)}
            placeholder="Any"
            step="0.1"
            type="number"
            value={filters.maxSizeMb}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="min-width"
            label="Minimum width"
            min={0}
            onChange={(event) => updateFilter("minWidth", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.minWidth}
          />

          <FieldInput
            id="min-height"
            label="Minimum height"
            min={0}
            onChange={(event) => updateFilter("minHeight", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.minHeight}
          />
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <FieldInput
            id="max-width"
            label="Maximum width"
            min={0}
            onChange={(event) => updateFilter("maxWidth", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.maxWidth}
          />

          <FieldInput
            id="max-height"
            label="Maximum height"
            min={0}
            onChange={(event) => updateFilter("maxHeight", event.target.value)}
            placeholder="Any"
            type="number"
            value={filters.maxHeight}
          />
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
    <Label className="flex w-full items-center gap-2 sm:w-auto">
      <span className="flex shrink-0 items-center gap-2 text-sm font-semibold text-neutral-800">
        <ArrowUpDown className="size-4 text-neutral-600" aria-hidden="true" />
        Sort
      </span>
      <NativeSelect
        className="min-w-48 flex-1 sm:flex-none"
        onChange={(event) => onChange(event.target.value as ResultSortMode)}
        value={value}
      >
        <option value="phash_distance">pHash distance</option>
        <option value="vector_score">Visual score</option>
        <option value="captured_newest">Newest captured</option>
        <option value="modified_newest">Newest modified</option>
        <option value="size_largest">Largest file</option>
        <option value="filename">Filename</option>
      </NativeSelect>
    </Label>
  );
}

function filterResults(results: SearchResult[], filters: MetadataFilters) {
  const cameraQuery = filters.cameraQuery.trim().toLocaleLowerCase();
  const keywordQuery = filters.keywordQuery.trim().toLocaleLowerCase();
  const nameQuery = filters.nameQuery.trim().toLocaleLowerCase();
  const personId = filters.personId.trim();
  const minSizeBytes = megabytesToBytes(positiveNumber(filters.minSizeMb));
  const maxSizeBytes = megabytesToBytes(positiveNumber(filters.maxSizeMb));
  const minWidth = positiveNumber(filters.minWidth);
  const minHeight = positiveNumber(filters.minHeight);
  const maxWidth = positiveNumber(filters.maxWidth);
  const maxHeight = positiveNumber(filters.maxHeight);
  const capturedFrom = dateBoundary(filters.captureDateFrom, "start");
  const capturedTo = dateBoundary(filters.captureDateTo, "end");
  const modifiedFrom = dateBoundary(filters.dateFrom, "start");
  const modifiedTo = dateBoundary(filters.dateTo, "end");

  return results.filter((result) => {
    const image = result.image;
    const photoMetadata = image.photo_metadata;

    if (nameQuery && !imageMatchesNameQuery(image, nameQuery)) {
      return false;
    }

    if (filters.sourceType !== "all" && image.source_type !== filters.sourceType) {
      return false;
    }

    if (filters.mediaKind !== "all" && image.media_kind !== filters.mediaKind) {
      return false;
    }

    if (cameraQuery && !photoMetadataMatchesCamera(photoMetadata, cameraQuery)) {
      return false;
    }

    if (keywordQuery && !photoMetadataMatchesKeyword(photoMetadata, keywordQuery)) {
      return false;
    }

    if (filters.hasGps === "yes" && !photoMetadata?.gps) {
      return false;
    }

    if (filters.hasGps === "no" && photoMetadata?.gps) {
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

    if (capturedFrom !== null || capturedTo !== null) {
      const capturedAt = captureTimeMs(photoMetadata?.capture_time ?? null);
      if (capturedAt === null) {
        return false;
      }
      if (capturedFrom !== null && capturedAt < capturedFrom) {
        return false;
      }
      if (capturedTo !== null && capturedAt > capturedTo) {
        return false;
      }
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
    case "captured_newest":
      return compareCapturedNewest(left, right);
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

function compareCapturedNewest(left: SearchResult, right: SearchResult) {
  const leftCaptured = captureTimeMs(left.image.photo_metadata?.capture_time ?? null);
  const rightCaptured = captureTimeMs(right.image.photo_metadata?.capture_time ?? null);

  if (leftCaptured === null && rightCaptured === null) {
    return compareHashDistanceForTie(left, right);
  }

  if (leftCaptured === null) {
    return 1;
  }

  if (rightCaptured === null) {
    return -1;
  }

  return rightCaptured - leftCaptured || compareHashDistanceForTie(left, right);
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

function photoMetadataMatchesCamera(
  metadata: SearchResult["image"]["photo_metadata"],
  cameraQuery: string,
) {
  if (!metadata) {
    return false;
  }

  return [metadata.camera_make, metadata.camera_model, metadata.lens_model].some((value) =>
    (value ?? "").toLocaleLowerCase().includes(cameraQuery),
  );
}

function photoMetadataMatchesKeyword(
  metadata: SearchResult["image"]["photo_metadata"],
  keywordQuery: string,
) {
  return (metadata?.keywords ?? []).some((keyword) =>
    keyword.toLocaleLowerCase().includes(keywordQuery),
  );
}

function captureTimeMs(value: string | null) {
  if (!value) {
    return null;
  }
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? null : parsed;
}

function imageOrientation(width: number, height: number): MetadataFilters["orientation"] {
  if (width === height) {
    return "square";
  }

  return width > height ? "landscape" : "portrait";
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
      item.queryMediaKind === "audio" ||
      item.queryMediaKind === "pdf") &&
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
    cameraQuery: stringFilter(partial.cameraQuery),
    captureDateFrom: stringFilter(partial.captureDateFrom),
    captureDateTo: stringFilter(partial.captureDateTo),
    dateFrom: stringFilter(partial.dateFrom),
    dateTo: stringFilter(partial.dateTo),
    hasGps: isHasGpsFilter(partial.hasGps) ? partial.hasGps : DEFAULT_METADATA_FILTERS.hasGps,
    keywordQuery: stringFilter(partial.keywordQuery),
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
    value === "audio" ||
    value === "pdf_page" ||
    value === "pdf_document"
  );
}

function isNearDuplicateFilter(value: unknown): value is MetadataFilters["nearDuplicate"] {
  return value === "all" || value === "exclude" || value === "only";
}

function isOrientationFilter(value: unknown): value is MetadataFilters["orientation"] {
  return value === "all" || value === "landscape" || value === "portrait" || value === "square";
}

function isHasGpsFilter(value: unknown): value is MetadataFilters["hasGps"] {
  return value === "all" || value === "yes" || value === "no";
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
          page_index: scene.page_index ?? null,
          page_number: scene.page_number ?? null,
          page_label: scene.page_label ?? null,
          results: Array.isArray(scene.results) ? scene.results.map(normalizeSearchResult) : [],
        }))
      : [],
  };
}

function removeResultFromResponse(response: SearchResponse, id: string): SearchResponse {
  const results = response.results.filter((result) => result.image.id !== id);
  return {
    ...response,
    count: results.length,
    results,
    scenes: response.scenes.map((scene) => {
      const sceneResults = scene.results.filter((result) => result.image.id !== id);
      return {
        ...scene,
        count: sceneResults.length,
        results: sceneResults,
      };
    }),
  };
}

function updateMediaInResponse(
  response: SearchResponse,
  media: SearchResult["image"],
): SearchResponse {
  const updateResult = (result: SearchResult): SearchResult =>
    result.image.id === media.id ? { ...result, image: media } : result;

  return {
    ...response,
    results: response.results.map(updateResult),
    scenes: response.scenes.map((scene) => ({
      ...scene,
      results: scene.results.map(updateResult),
    })),
  };
}

function normalizeSearchResult(result: SearchResult): SearchResult {
  return {
    ...result,
    image: {
      ...result.image,
      faces: Array.isArray(result.image.faces) ? result.image.faces : [],
      people: Array.isArray(result.image.people) ? result.image.people : [],
      full_pdf_url: result.image.full_pdf_url ?? null,
      pdf_page_url: result.image.pdf_page_url ?? null,
      pdf_document_id: result.image.pdf_document_id ?? null,
      pdf_page_index: result.image.pdf_page_index ?? null,
      pdf_page_number: result.image.pdf_page_number ?? null,
      pdf_page_count: result.image.pdf_page_count ?? null,
      visual_embedding_model: result.image.visual_embedding_model ?? null,
      artifacts: Array.isArray(result.image.artifacts) ? result.image.artifacts : [],
      tags: Array.isArray(result.image.tags) ? result.image.tags : [],
      photo_metadata: normalizePhotoMetadata(result.image.photo_metadata),
    },
  };
}

function parseTagDraft(value: string) {
  const seen = new Set<string>();
  const tags: string[] = [];

  for (const rawTag of value.split(",")) {
    const tag = rawTag.trim();
    const key = tag.toLocaleLowerCase();
    if (!tag || seen.has(key)) {
      continue;
    }
    seen.add(key);
    tags.push(tag);
  }

  return tags;
}

function sameTags(left: string[], right: string[]) {
  return left.length === right.length && left.every((tag, index) => tag === right[index]);
}

function normalizePhotoMetadata(metadata: SearchResult["image"]["photo_metadata"]) {
  if (!metadata || typeof metadata !== "object") {
    return null;
  }

  return {
    ...metadata,
    gps: metadata.gps
      ? {
          ...metadata.gps,
          altitude_meters: metadata.gps.altitude_meters ?? null,
        }
      : null,
    keywords: Array.isArray(metadata.keywords) ? metadata.keywords : [],
    raw: Array.isArray(metadata.raw) ? metadata.raw : [],
  };
}

function normalizeResultSortMode(value: unknown): ResultSortMode {
  return isResultSortMode(value) ? value : DEFAULT_RESULT_SORT;
}

function isResultSortMode(value: unknown): value is ResultSortMode {
  return (
    value === "captured_newest" ||
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
    <Alert
      variant={tone === "error" ? "destructive" : "default"}
      className={`flex min-h-11 items-start gap-2 rounded-md border px-3 py-2 text-sm ${toneClass}`}
    >
      <span className="mt-0.5 shrink-0" aria-hidden="true">
        {icon}
      </span>
      <AlertDescription>{text}</AlertDescription>
    </Alert>
  );
}

function ResultsGrid({
  deletingId,
  onDelete,
  onUpdateTags,
  pending,
  results,
  searched,
  tagSavingId,
}: {
  deletingId?: string;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  pending: boolean;
  results: SearchResult[];
  searched: boolean;
  tagSavingId?: string;
}) {
  if (pending) {
    return (
      <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600">
        <Loader2 className="size-7 animate-spin" aria-label="Loading search results" />
      </div>
    );
  }

  if (!searched) {
    return <EmptyResults text="Choose a query image, video, audio, or PDF and run a search." />;
  }

  if (results.length === 0) {
    return <EmptyResults text="No indexed media matched this query." />;
  }

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {results.map((result) => (
        <ResultCard
          deleting={deletingId === result.image.id}
          key={result.image.id}
          onDelete={onDelete}
          onUpdateTags={onUpdateTags}
          result={result}
          tagSaving={tagSavingId === result.image.id}
        />
      ))}
    </div>
  );
}

function SceneResultsList({
  deletingId,
  filters,
  onDelete,
  onUpdateTags,
  onSelectScene,
  resultLimit,
  scenes,
  selectedSceneIndex,
  sortMode,
  tagSavingId,
}: {
  deletingId?: string;
  filters: MetadataFilters;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  onSelectScene: (sceneIndex: number) => void;
  resultLimit: number;
  scenes: SearchSceneResponse[];
  selectedSceneIndex: number | null;
  sortMode: ResultSortMode;
  tagSavingId?: string;
}) {
  const selectedScene =
    scenes.find((scene) => scene.scene_index === selectedSceneIndex) ?? scenes[0];
  const selectedResults = selectedScene
    ? sortResults(filterResults(selectedScene.results, filters), sortMode).slice(0, resultLimit)
    : [];
  const isAudioBits = scenes.some((scene) => scene.scene_kind === "audio_bit");
  const isPdfPages = scenes.some((scene) => scene.scene_kind === "pdf_page");
  const segmentLabel = isPdfPages ? "Page" : isAudioBits ? "Bit" : "Scene";
  const SegmentIcon = isPdfPages ? FileText : isAudioBits ? FileAudio : FileVideo;

  return (
    <div className="flex flex-col gap-5">
      <div className="rounded-lg border border-neutral-300 bg-white p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2 text-sm font-semibold text-neutral-950">
          <SegmentIcon className="size-4 text-neutral-600" aria-hidden="true" />
          <span>Query segment</span>
        </div>
        <div className="flex gap-2 overflow-x-auto pb-1">
          {scenes.map((scene) => (
            <Button
              aria-pressed={scene.scene_index === selectedScene?.scene_index}
              variant={scene.scene_index === selectedScene?.scene_index ? "default" : "outline"}
              className={`inline-flex h-10 shrink-0 items-center justify-center rounded-md border px-3 text-sm font-semibold transition ${
                scene.scene_index === selectedScene?.scene_index
                  ? "border-emerald-700 bg-emerald-50 text-emerald-950"
                  : "border-neutral-300 bg-white text-neutral-800 hover:border-neutral-500 hover:bg-neutral-50"
              }`}
              key={scene.scene_index}
              onClick={() => onSelectScene(scene.scene_index)}
              type="button"
            >
              {scene.page_label ?? `${segmentLabel} ${scene.scene_index + 1}`}
              {!isPdfPages
                ? ` · ${formatSeconds(scene.start_seconds)}-${formatSeconds(scene.end_seconds)}`
                : ""}
              {scene.speaker_label ? ` · ${scene.speaker_label}` : ""}
            </Button>
          ))}
        </div>
      </div>

      {selectedScene ? (
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="mb-3 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <h3 className="text-sm font-semibold text-neutral-950">
                {selectedScene.page_label ?? `${segmentLabel} ${selectedScene.scene_index + 1}`}
              </h3>
              <p className="text-xs text-neutral-600">
                {isPdfPages
                  ? selectedScene.page_number
                    ? `Page ${selectedScene.page_number}`
                    : "PDF page"
                  : `${formatSeconds(selectedScene.start_seconds)}-${formatSeconds(
                      selectedScene.end_seconds,
                    )}`}
                {!isPdfPages && isAudioBits
                  ? selectedScene.speaker_label
                    ? ` · ${selectedScene.speaker_label}`
                    : ""
                  : !isPdfPages
                    ? ` · frames ${selectedScene.start_frame}-${selectedScene.end_frame}`
                    : ""}
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
          <ResultsGrid
            deletingId={deletingId}
            onDelete={onDelete}
            onUpdateTags={onUpdateTags}
            pending={false}
            results={selectedResults}
            searched
            tagSavingId={tagSavingId}
          />
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

function PhotoMetadataDetails({
  metadata,
}: {
  metadata: NonNullable<SearchResult["image"]["photo_metadata"]>;
}) {
  return (
    <details className="rounded-md border border-neutral-200 bg-neutral-50 p-3 text-sm">
      <summary className="cursor-pointer font-semibold text-neutral-800">Photo metadata</summary>
      <dl className="mt-3 grid gap-2">
        {metadata.raw.map((entry, index) => (
          <div className="grid gap-1" key={`${entry.namespace}-${entry.key}-${index}`}>
            <dt className="text-xs font-semibold uppercase text-neutral-500">
              {entry.namespace} · {entry.label || entry.key}
            </dt>
            <dd className="break-words text-neutral-900">{entry.value}</dd>
          </div>
        ))}
      </dl>
    </details>
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

function PdfLinks({ image }: { image: SearchResult["image"] }) {
  if (image.media_kind !== "pdf_page" && image.media_kind !== "pdf_document") {
    return null;
  }

  const pageUrl =
    image.pdf_page_url ??
    (image.full_pdf_url && image.pdf_page_number
      ? `${image.full_pdf_url}#page=${image.pdf_page_number}`
      : null);

  return (
    <div className="grid gap-2">
      {image.pdf_page_number ? (
        <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
          Page {image.pdf_page_number}
          {image.pdf_page_count ? ` of ${image.pdf_page_count}` : ""}
        </div>
      ) : image.pdf_page_count ? (
        <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
          {image.pdf_page_count} page(s)
        </div>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {image.full_pdf_url ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={image.full_pdf_url}
            rel="noreferrer"
            target="_blank"
          >
            <FileText className="size-3.5" aria-hidden="true" />
            <span>Open PDF</span>
          </a>
        ) : null}
        {pageUrl ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={pageUrl}
            rel="noreferrer"
            target="_blank"
          >
            <FileText className="size-3.5" aria-hidden="true" />
            <span>Open page</span>
          </a>
        ) : null}
      </div>
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

function ResultCard({
  deleting = false,
  onDelete,
  onUpdateTags,
  result,
  tagSaving = false,
}: {
  deleting?: boolean;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  result: SearchResult;
  tagSaving?: boolean;
}) {
  const image = result.image;
  const faces = image.faces ?? [];
  const people = image.people ?? [];
  const photoMetadata = image.photo_metadata;
  const cameraLabel = photoMetadata ? photoCameraLabel(photoMetadata) : null;
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
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold text-neutral-950" title={image.filename}>
              {image.filename}
            </h3>
            <p className="mt-1 truncate text-xs text-neutral-600" title={image.relative_path}>
              {image.relative_path}
            </p>
          </div>
          {onDelete ? (
            <Button
              aria-label={`Delete ${image.filename} from index`}
              variant="outline"
              className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-red-300 hover:bg-red-50 hover:text-red-700 disabled:cursor-wait disabled:opacity-60"
              disabled={deleting}
              onClick={() => {
                if (window.confirm(`Delete ${image.filename} from the index?`)) {
                  onDelete(image.id);
                }
              }}
              title="Delete from index"
              type="button"
            >
              {deleting ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Trash2 className="size-4" aria-hidden="true" />
              )}
            </Button>
          ) : null}
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
          {image.pdf_page_number ? (
            <Metric
              label="PDF page"
              value={
                image.pdf_page_count
                  ? `${image.pdf_page_number} of ${image.pdf_page_count}`
                  : image.pdf_page_number
              }
            />
          ) : image.pdf_page_count ? (
            <Metric label="PDF pages" value={image.pdf_page_count} />
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
          {photoMetadata?.capture_time ? (
            <Metric label="Captured" value={formatCaptureTime(photoMetadata.capture_time)} />
          ) : null}
          {cameraLabel ? <Metric label="Camera" value={cameraLabel} /> : null}
          {photoMetadata?.lens_model ? (
            <Metric label="Lens" value={photoMetadata.lens_model} />
          ) : null}
          {photoMetadata?.gps ? <Metric label="GPS" value={formatGps(photoMetadata.gps)} /> : null}
          {photoMetadata?.rating !== null && photoMetadata?.rating !== undefined ? (
            <Metric label="Rating" value={photoMetadata.rating} />
          ) : null}
          {photoMetadata?.keywords?.length ? (
            <Metric label="Keywords" value={photoMetadata.keywords.join(", ")} />
          ) : null}
          {photoMetadata?.creator ? <Metric label="Creator" value={photoMetadata.creator} /> : null}
          {photoMetadata?.copyright ? (
            <Metric label="Copyright" value={photoMetadata.copyright} />
          ) : null}
          {image.ocr_text ? <Metric label="OCR text" value={image.ocr_text} /> : null}
          {faces.length ? <Metric label="Faces" value={faces.length} /> : null}
          {people.length ? (
            <Metric label="People" value={people.map(personDisplayName).join(", ")} />
          ) : null}
        </dl>

        <MediaTagEditor image={image} onUpdateTags={onUpdateTags} saving={tagSaving} />

        {photoMetadata?.raw?.length ? <PhotoMetadataDetails metadata={photoMetadata} /> : null}

        <VideoSceneLinks image={image} />
        <AudioLinks image={image} />
        <PdfLinks image={image} />

        <div className="flex flex-wrap gap-2">
          {image.media_kind === "animated_gif" ? (
            <Tag className="border-sky-300 bg-sky-50 text-sky-900">GIF</Tag>
          ) : null}
          {image.media_kind === "video_scene" ? (
            <Tag className="border-violet-300 bg-violet-50 text-violet-900">Video scene</Tag>
          ) : null}
          {image.media_kind === "audio" ? (
            <Tag className="border-emerald-300 bg-emerald-50 text-emerald-900">Audio</Tag>
          ) : null}
          {image.media_kind === "pdf_document" ? (
            <Tag className="border-red-300 bg-red-50 text-red-900">PDF document</Tag>
          ) : null}
          {image.media_kind === "pdf_page" ? (
            <Tag className="border-orange-300 bg-orange-50 text-orange-900">PDF page</Tag>
          ) : null}
          {image.ocr_text ? (
            <Tag className="border-cyan-300 bg-cyan-50 text-cyan-900">OCR</Tag>
          ) : null}
          {image.audio_analysis?.speech_detected ? (
            <Tag className="border-teal-300 bg-teal-50 text-teal-900">Speech</Tag>
          ) : null}
          {image.audio_analysis?.recognized_voices?.map((voice) => (
            <Tag className="border-lime-300 bg-lime-50 text-lime-900" key={voice.id}>
              {voice.label}
            </Tag>
          ))}
          {image.audio_analysis?.transcript_text ? (
            <Tag className="border-fuchsia-300 bg-fuchsia-50 text-fuchsia-900">Transcript</Tag>
          ) : null}
          {image.audio_analysis?.tempo_bpm ? (
            <Tag className="border-rose-300 bg-rose-50 text-rose-900">
              {image.audio_analysis.tempo_bpm.toFixed(0)} BPM
            </Tag>
          ) : null}
          {faces.length ? (
            <Tag className="border-indigo-300 bg-indigo-50 text-indigo-900">
              Faces {faces.length}
            </Tag>
          ) : null}
          {people.map((person) => (
            <Tag
              className="border-purple-300 bg-purple-50 text-purple-900"
              key={person.person_id}
              title={person.person_id}
            >
              {personDisplayName(person)}
            </Tag>
          ))}
          {result.query_scene_index !== null && result.query_scene_index !== undefined ? (
            <Tag className="border-neutral-300 bg-neutral-50 text-neutral-700">
              Query scene {result.query_scene_index + 1}
            </Tag>
          ) : null}
          {result.near_duplicate ? (
            <Tag className="border-amber-300 bg-amber-50 text-amber-900">Near duplicate</Tag>
          ) : null}
        </div>
      </div>
    </article>
  );
}

function MediaTagEditor({
  image,
  onUpdateTags,
  saving,
}: {
  image: SearchResult["image"];
  onUpdateTags?: (id: string, tags: string[]) => void;
  saving: boolean;
}) {
  const tags = image.tags ?? [];
  const [draft, setDraft] = useState(tags.join(", "));

  useEffect(() => {
    setDraft(tags.join(", "));
  }, [image.id, tags.join("\u0000")]);

  const draftTags = parseTagDraft(draft);
  const dirty = !sameTags(draftTags, tags);

  function removeTag(tag: string) {
    setDraft(draftTags.filter((item) => item !== tag).join(", "));
  }

  return (
    <form
      className="grid gap-2 rounded-md border border-neutral-200 bg-neutral-50 p-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (onUpdateTags && dirty && !saving) {
          onUpdateTags(image.id, draftTags);
        }
      }}
    >
      <div className="flex items-center justify-between gap-2">
        <Label
          className="text-xs font-semibold uppercase text-neutral-500"
          htmlFor={`tags-${image.id}`}
        >
          Tags
        </Label>
        <Button
          aria-label={`Save tags for ${image.filename}`}
          className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={!onUpdateTags || !dirty || saving}
          type="submit"
          variant="outline"
        >
          {saving ? (
            <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <Save className="size-3.5" aria-hidden="true" />
          )}
          <span>Save</span>
        </Button>
      </div>
      <Input
        aria-label={`Tags for ${image.filename}`}
        className="h-9 rounded-md border-neutral-300 bg-white text-sm"
        disabled={!onUpdateTags || saving}
        id={`tags-${image.id}`}
        onChange={(event) => setDraft(event.target.value)}
        placeholder="travel, family"
        value={draft}
      />
      {draftTags.length ? (
        <div className="flex flex-wrap gap-1.5">
          {draftTags.map((tag) => (
            <Badge
              className="inline-flex max-w-full items-center gap-1 rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-800"
              key={tag}
              variant="outline"
            >
              <span className="truncate">{tag}</span>
              <button
                aria-label={`Remove tag ${tag}`}
                className="inline-grid size-4 shrink-0 place-items-center rounded text-neutral-500 transition hover:bg-neutral-200 hover:text-neutral-950"
                disabled={!onUpdateTags || saving}
                onClick={() => removeTag(tag)}
                type="button"
              >
                <X className="size-3" aria-hidden="true" />
              </button>
            </Badge>
          ))}
        </div>
      ) : null}
    </form>
  );
}

function Tag({
  children,
  className = "",
  title,
}: {
  children: React.ReactNode;
  className?: string;
  title?: string;
}) {
  return (
    <Badge
      className={`inline-flex w-fit rounded-md border px-2 py-1 text-xs font-semibold ${className}`}
      title={title}
      variant="outline"
    >
      {children}
    </Badge>
  );
}

function countActiveFilters(filters: MetadataFilters) {
  return Object.entries(filters).filter(([key, value]) => {
    const defaultValue = DEFAULT_METADATA_FILTERS[key as keyof MetadataFilters];
    return value !== defaultValue;
  }).length;
}

function sortPeopleEntries(people: InverseIndexResponse["people"]) {
  return [...people].sort(
    (left, right) =>
      right.media_count - left.media_count ||
      right.face_count - left.face_count ||
      registryName(left).localeCompare(registryName(right), undefined, {
        sensitivity: "base",
      }),
  );
}

function sortSpeakerEntries(speakers: InverseIndexResponse["speakers"]) {
  return [...speakers].sort(
    (left, right) =>
      right.media_count - left.media_count ||
      right.total_seconds - left.total_seconds ||
      registryName(left).localeCompare(registryName(right), undefined, {
        sensitivity: "base",
      }),
  );
}

function registryName(entry: { id: string; label: string | null }) {
  return entry.label?.trim() || entry.id;
}

function compactNumberList(values: number[]) {
  if (values.length <= 4) {
    return values.join(", ");
  }

  return `${values.slice(0, 4).join(", ")} +${values.length - 4}`;
}

function formatDuration(durationMs: number) {
  return `${(durationMs / 1000).toFixed(1)}s`;
}

function formatDurationSeconds(seconds: number) {
  if (seconds < 60) {
    return `${seconds.toFixed(1)}s total`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = Math.round(seconds % 60);
  return `${minutes}m ${remainingSeconds}s total`;
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

function formatCaptureTime(value: string) {
  const parsed = captureTimeMs(value);
  if (parsed === null) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).format(new Date(parsed));
}

function photoCameraLabel(metadata: NonNullable<SearchResult["image"]["photo_metadata"]>) {
  return [metadata.camera_make, metadata.camera_model].filter(Boolean).join(" ") || null;
}

function formatGps(gps: NonNullable<SearchResult["image"]["photo_metadata"]>["gps"]) {
  if (!gps) {
    return "";
  }

  const coordinates = `${gps.latitude.toFixed(5)}, ${gps.longitude.toFixed(5)}`;
  return gps.altitude_meters !== null && gps.altitude_meters !== undefined
    ? `${coordinates}, ${gps.altitude_meters.toFixed(1)} m`
    : coordinates;
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-neutral-600">{label}</dt>
      <dd className="min-w-0 truncate font-medium text-neutral-900">{value}</dd>
    </div>
  );
}

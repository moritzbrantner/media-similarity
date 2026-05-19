import { useMutation, useQuery } from "@tanstack/react-query";
import {
  AlertCircle,
  CheckCircle2,
  Database,
  FileImage,
  History,
  ImageIcon,
  Loader2,
  RotateCw,
  Search,
  SlidersHorizontal,
  Upload,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";
import { fetchHealth, indexSources, searchImage } from "./api";
import type { IndexResponse, SearchResponse, SearchResult } from "./types";

const DEFAULT_LIMIT = 12;
const MAX_SEARCH_HISTORY = 8;
const SEARCH_HISTORY_STORAGE_KEY = "image-similarity-search-history";

const DEFAULT_METADATA_FILTERS = {
  minHeight: "",
  minWidth: "",
  nearDuplicate: "all",
  orientation: "all",
  sourceType: "all",
} satisfies MetadataFilters;

type MetadataFilters = {
  minHeight: string;
  minWidth: string;
  nearDuplicate: "all" | "exclude" | "only";
  orientation: "all" | "landscape" | "portrait" | "square";
  sourceType: string;
};

type SearchHistoryItem = {
  id: string;
  fileName: string;
  filters: MetadataFilters;
  limit: number;
  queryImageUrl: string | null;
  searchedAt: string;
  response: SearchResponse;
};

type SearchVariables = {
  filters: MetadataFilters;
  queryFile: File;
  queryImageUrl: string | null;
  resultLimit: number;
};

export function App() {
  const [file, setFile] = useState<File | null>(null);
  const [limit, setLimit] = useState(DEFAULT_LIMIT);
  const [metadataFilters, setMetadataFilters] = useState<MetadataFilters>(DEFAULT_METADATA_FILTERS);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [lastIndex, setLastIndex] = useState<IndexResponse | null>(null);
  const [searchHistory, setSearchHistory] = useState<SearchHistoryItem[]>(loadSearchHistory);
  const [activeSearchId, setActiveSearchId] = useState<string | null>(null);

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
  });

  const indexMutation = useMutation({
    mutationFn: indexSources,
    onSuccess: (response) => {
      setLastIndex(response);
    },
  });

  const searchMutation = useMutation({
    mutationFn: ({ queryFile, resultLimit }: SearchVariables) =>
      searchImage(queryFile, resultLimit),
    onSuccess: (response, variables) => {
      const nextItem: SearchHistoryItem = {
        id: createHistoryId(),
        fileName: variables.queryFile.name,
        filters: variables.filters,
        limit: variables.resultLimit,
        queryImageUrl: variables.queryImageUrl,
        searchedAt: new Date().toISOString(),
        response,
      };

      setSearchHistory((history) => [nextItem, ...history].slice(0, MAX_SEARCH_HISTORY));
      setActiveSearchId(nextItem.id);
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
    const queryImageUrl = await createQueryPreview(file);
    searchMutation.mutate({
      filters: metadataFilters,
      queryFile: file,
      queryImageUrl,
      resultLimit: limit,
    });
  }

  function handleFileChange(nextFile: File | null) {
    setFile(nextFile);
    setActiveSearchId(null);
    searchMutation.reset();
  }

  const activeSearch = searchHistory.find((item) => item.id === activeSearchId) ?? null;
  const activeResponse = activeSearch?.response ?? null;
  const displayedPreviewUrl = activeSearch?.queryImageUrl ?? previewUrl;
  const sourceTypeOptions = sourceTypesFor(
    activeResponse?.results ?? [],
    metadataFilters.sourceType,
  );
  const results = filterResults(activeResponse?.results ?? [], metadataFilters);

  function handleHistorySelect(item: SearchHistoryItem) {
    setActiveSearchId(item.id);
    setLimit(item.limit);
    setMetadataFilters(item.filters);
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

          <button
            className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-400 bg-white px-4 text-sm font-semibold text-neutral-900 shadow-sm transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
            disabled={indexMutation.isPending}
            onClick={() => indexMutation.mutate()}
            type="button"
          >
            {indexMutation.isPending ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <Database className="size-4" aria-hidden="true" />
            )}
            <span>Index Sources</span>
          </button>
        </header>

        <section className="grid gap-5 lg:grid-cols-[360px_minmax(0,1fr)]">
          <form
            className="flex flex-col gap-4 rounded-lg border border-neutral-300 bg-white p-4 shadow-sm"
            onSubmit={handleSubmit}
          >
            <div>
              <label className="text-sm font-semibold text-neutral-900" htmlFor="query-image">
                Query image
              </label>
              <label
                className="mt-2 flex min-h-32 cursor-pointer flex-col items-center justify-center gap-2 rounded-md border border-dashed border-neutral-400 bg-neutral-50 px-4 py-5 text-center transition hover:border-emerald-600 hover:bg-emerald-50"
                htmlFor="query-image"
              >
                <Upload className="size-6 text-neutral-600" aria-hidden="true" />
                <span className="max-w-full truncate text-sm font-medium text-neutral-800">
                  {file?.name ?? "Choose an image"}
                </span>
                <span className="text-xs text-neutral-500">PNG, JPEG, WebP, BMP, or TIFF</span>
              </label>
              <input
                accept="image/*"
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
                onChange={(event) => setLimit(Number(event.target.value || DEFAULT_LIMIT))}
                type="number"
                value={limit}
              />
            </div>

            <MetadataFiltersPanel
              filters={metadataFilters}
              onChange={setMetadataFilters}
              sourceTypeOptions={sourceTypeOptions}
            />

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
                  aria-label="Clear selected image"
                  className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-neutral-500 hover:bg-neutral-50"
                  onClick={() => handleFileChange(null)}
                  title="Clear selected image"
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
              <img
                alt="Query preview"
                className="h-full max-h-[420px] w-full object-contain"
                src={displayedPreviewUrl}
              />
            ) : (
              <div className="flex flex-col items-center justify-center gap-3 bg-neutral-50 p-8 text-center text-neutral-500">
                <ImageIcon className="size-12" aria-hidden="true" />
                <span className="text-sm font-medium">No query image selected</span>
              </div>
            )}
          </section>
        </section>

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
                  {activeResponse
                    ? `${results.length} of ${activeResponse.count} result(s), query pHash ${activeResponse.query_phash}`
                    : searchMutation.isPending
                      ? "Searching indexed images."
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
            </div>

            <ResultsGrid
              pending={searchMutation.isPending}
              results={results}
              searched={Boolean(activeResponse)}
            />
          </div>
        </section>
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

  return (
    <fieldset className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <legend className="flex items-center gap-2 px-1 text-sm font-semibold text-neutral-900">
        <SlidersHorizontal className="size-4 text-neutral-600" aria-hidden="true" />
        <span>Metadata filters</span>
      </legend>

      <div className="mt-3 grid gap-3">
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

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
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

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
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
      </div>
    </fieldset>
  );
}

function filterResults(results: SearchResult[], filters: MetadataFilters) {
  const minWidth = positiveNumber(filters.minWidth);
  const minHeight = positiveNumber(filters.minHeight);

  return results.filter((result) => {
    const image = result.image;

    if (filters.sourceType !== "all" && image.source_type !== filters.sourceType) {
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

    return true;
  });
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

function imageOrientation(width: number, height: number): MetadataFilters["orientation"] {
  if (width === height) {
    return "square";
  }

  return width > height ? "landscape" : "portrait";
}

async function createQueryPreview(file: File) {
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
        queryImageUrl: item.queryImageUrl ?? null,
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
    (item.filters === undefined || isMetadataFilters(item.filters)) &&
    typeof item.limit === "number" &&
    (typeof item.queryImageUrl === "string" ||
      item.queryImageUrl === null ||
      item.queryImageUrl === undefined) &&
    typeof item.searchedAt === "string" &&
    Boolean(response) &&
    Array.isArray(response?.results) &&
    typeof response?.count === "number" &&
    typeof response?.query_phash === "string"
  );
}

function normalizeMetadataFilters(filters: unknown): MetadataFilters {
  if (!isMetadataFilters(filters)) {
    return DEFAULT_METADATA_FILTERS;
  }

  return filters;
}

function isMetadataFilters(value: unknown): value is MetadataFilters {
  if (!value || typeof value !== "object") {
    return false;
  }

  const filters = value as Partial<MetadataFilters>;
  return (
    typeof filters.minHeight === "string" &&
    typeof filters.minWidth === "string" &&
    isNearDuplicateFilter(filters.nearDuplicate) &&
    isOrientationFilter(filters.orientation) &&
    typeof filters.sourceType === "string"
  );
}

function isNearDuplicateFilter(value: unknown): value is MetadataFilters["nearDuplicate"] {
  return value === "all" || value === "exclude" || value === "only";
}

function isOrientationFilter(value: unknown): value is MetadataFilters["orientation"] {
  return value === "all" || value === "landscape" || value === "portrait" || value === "square";
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
                  <span>{item.response.count} result(s)</span>
                </span>
                <span className="text-xs text-neutral-500">Limit {item.limit}</span>
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
        text="Searching indexed images."
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
    const text = `Indexed ${lastIndex.indexed} image(s), skipped ${lastIndex.skipped}, failed ${lastIndex.failed}.`;
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
    return <EmptyResults text="Choose a query image and run a search." />;
  }

  if (results.length === 0) {
    return <EmptyResults text="No indexed images matched this query." />;
  }

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {results.map((result) => (
        <ResultCard key={result.image.id} result={result} />
      ))}
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

  return (
    <article className="overflow-hidden rounded-lg border border-neutral-300 bg-white shadow-sm">
      <div className="grid aspect-[4/3] place-items-center bg-neutral-200">
        {image.thumbnail_url ? (
          <img
            alt=""
            className="h-full w-full object-contain"
            loading="lazy"
            src={image.thumbnail_url}
          />
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
          <Metric label="CLIP score" value={result.vector_score.toFixed(4)} />
          <Metric label="pHash distance" value={result.hash_distance ?? "n/a"} />
          <Metric label="Size" value={`${image.width} x ${image.height}`} />
        </dl>

        {result.near_duplicate ? (
          <span className="inline-flex w-fit rounded-md border border-amber-300 bg-amber-50 px-2 py-1 text-xs font-semibold text-amber-900">
            Near duplicate
          </span>
        ) : null}
      </div>
    </article>
  );
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-neutral-600">{label}</dt>
      <dd className="min-w-0 truncate font-medium text-neutral-900">{value}</dd>
    </div>
  );
}

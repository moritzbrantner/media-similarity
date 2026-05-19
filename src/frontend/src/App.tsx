import { useMutation, useQuery } from "@tanstack/react-query";
import {
  AlertCircle,
  CheckCircle2,
  Database,
  FileImage,
  ImageIcon,
  Loader2,
  RotateCw,
  Search,
  Upload,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";
import { fetchHealth, indexSources, searchImage } from "./api";
import type { IndexResponse, SearchResult } from "./types";

const DEFAULT_LIMIT = 12;

export function App() {
  const [file, setFile] = useState<File | null>(null);
  const [limit, setLimit] = useState(DEFAULT_LIMIT);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [lastIndex, setLastIndex] = useState<IndexResponse | null>(null);

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
    mutationFn: ({ queryFile, resultLimit }: { queryFile: File; resultLimit: number }) =>
      searchImage(queryFile, resultLimit),
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

  const sourcesLabel = useMemo(() => {
    const health = healthQuery.data;
    if (!health) {
      return healthQuery.isError ? "Service is not responding" : "Checking service status";
    }

    const sources = health.sources.length > 0 ? health.sources : [health.source_dir];
    return sources.join(", ");
  }, [healthQuery.data, healthQuery.isError]);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!file) {
      return;
    }

    searchMutation.mutate({ queryFile: file, resultLimit: limit });
  }

  function handleFileChange(nextFile: File | null) {
    setFile(nextFile);
    searchMutation.reset();
  }

  const results = searchMutation.data?.results ?? [];

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
            {previewUrl ? (
              <img
                alt="Selected query preview"
                className="h-full max-h-[420px] w-full object-contain"
                src={previewUrl}
              />
            ) : (
              <div className="flex flex-col items-center justify-center gap-3 bg-neutral-50 p-8 text-center text-neutral-500">
                <ImageIcon className="size-12" aria-hidden="true" />
                <span className="text-sm font-medium">No query image selected</span>
              </div>
            )}
          </section>
        </section>

        <section className="flex flex-col gap-3">
          <div className="flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <h2 className="text-lg font-semibold text-neutral-950">Results</h2>
              <p className="text-sm text-neutral-600">
                {searchMutation.data
                  ? `${searchMutation.data.count} result(s), query pHash ${searchMutation.data.query_phash}`
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
            searched={Boolean(searchMutation.data)}
          />
        </section>
      </div>
    </main>
  );
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

import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { extractAudioSignature, extractVideoSignature } from "./mediaFeatures";
import {
  createModeRuntimes,
  disposeModeRuntimes,
  EMPTY_GALLERIES,
  inputForSample,
  MODES,
  queryForSample,
  rankGallery,
  type MediaKind,
  type ModeGalleries,
  type ModeRuntimes,
  type QueryPreview,
  type RankedSample,
  type ShowcaseSample,
  type SimilarityInput,
} from "./showcaseRuntime";

function QueryMedia({ query }: { query: QueryPreview }) {
  if (query.kind === "audio") {
    return (
      <div className="space-y-4">
        {query.previewUrl ? (
          <img
            src={query.previewUrl}
            alt=""
            className="aspect-[8/5] w-full rounded-2xl object-cover"
          />
        ) : (
          <div className="flex aspect-[8/5] w-full items-center justify-center rounded-2xl border border-neutral-800 bg-neutral-950 text-sm text-neutral-500">
            Browser-decoded audio query
          </div>
        )}
        {query.mediaUrl ? <audio className="w-full" controls src={query.mediaUrl} /> : null}
      </div>
    );
  }

  if (query.kind === "video" && query.mediaUrl) {
    return (
      <video
        className="aspect-[8/5] w-full rounded-2xl bg-black object-contain"
        controls
        muted
        playsInline
        preload="metadata"
        src={query.mediaUrl}
      />
    );
  }

  return query.previewUrl ? (
    <img
      src={query.previewUrl}
      alt={query.label}
      className="aspect-[8/5] w-full rounded-2xl object-cover"
    />
  ) : null;
}

function SamplePicker({
  gallery,
  query,
  onSelect,
}: {
  gallery: ShowcaseSample[];
  query: QueryPreview | null;
  onSelect: (sample: ShowcaseSample) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-2">
      {gallery.map((sample) => (
        <button
          type="button"
          key={sample.id}
          onClick={() => onSelect(sample)}
          className={`overflow-hidden rounded-xl border text-left transition ${
            query?.id === sample.id
              ? "border-emerald-400 bg-emerald-400/10"
              : "border-neutral-800 bg-neutral-950 hover:border-neutral-600"
          }`}
        >
          <img src={sample.previewUrl} alt="" className="aspect-[8/5] w-full object-cover" />
          <div className="px-3 py-2">
            <div className="text-xs font-medium text-neutral-200">{sample.label}</div>
            <div className="text-[11px] text-neutral-500">{sample.family}</div>
          </div>
        </button>
      ))}
    </div>
  );
}

function RankedResults({ results }: { results: RankedSample[] }) {
  return (
    <div className="grid gap-3 sm:grid-cols-2">
      {results.slice(0, 6).map(({ sample, score }, index) => (
        <article
          key={sample.id}
          className="grid grid-cols-[112px_1fr] overflow-hidden rounded-2xl border border-neutral-800 bg-neutral-950"
        >
          <img
            src={sample.previewUrl}
            alt={sample.label}
            className="h-full min-h-28 w-full object-cover"
          />
          <div className="flex flex-col justify-between gap-3 p-4">
            <div>
              <div className="text-xs text-neutral-500">#{index + 1}</div>
              <div className="mt-1 text-sm font-medium text-white">{sample.label}</div>
              <div className="mt-1 text-[11px] text-neutral-500">{sample.family}</div>
            </div>
            <div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-neutral-500">similarity</span>
                <span className="font-mono text-neutral-200">{(score * 100).toFixed(1)}%</span>
              </div>
              <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-neutral-800">
                <div
                  className="h-full rounded-full bg-emerald-400"
                  style={{ width: `${Math.max(2, score * 100)}%` }}
                />
              </div>
            </div>
          </div>
        </article>
      ))}
    </div>
  );
}

export function MediaDemo() {
  const runtimesRef = useRef<ModeRuntimes | null>(null);
  const uploadedUrlRef = useRef<string | null>(null);
  const [activeKind, setActiveKind] = useState<MediaKind>("image");
  const [galleries, setGalleries] = useState<ModeGalleries>(EMPTY_GALLERIES);
  const [query, setQuery] = useState<QueryPreview | null>(null);
  const [results, setResults] = useState<RankedSample[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [queryBusy, setQueryBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function releaseUploadedUrl() {
    if (uploadedUrlRef.current) {
      URL.revokeObjectURL(uploadedUrlRef.current);
      uploadedUrlRef.current = null;
    }
  }

  useEffect(() => {
    let cancelled = false;

    createModeRuntimes()
      .then((runtimes) => {
        if (cancelled) {
          disposeModeRuntimes(runtimes);
          return;
        }

        const first = runtimes.image.gallery[0];
        runtimesRef.current = runtimes;
        setGalleries({
          audio: runtimes.audio.gallery,
          image: runtimes.image.gallery,
          video: runtimes.video.gallery,
        });
        setQuery(queryForSample(first));
        setResults(rankGallery(runtimes.image, inputForSample(first), first.id));
        setStatus("ready");
      })
      .catch((cause) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
          setStatus("error");
        }
      });

    return () => {
      cancelled = true;
      releaseUploadedUrl();
      if (runtimesRef.current) {
        disposeModeRuntimes(runtimesRef.current);
        runtimesRef.current = null;
      }
    };
  }, []);

  function selectMode(kind: MediaKind) {
    const runtime = runtimesRef.current?.[kind];
    if (!runtime || queryBusy) {
      return;
    }

    const first = runtime.gallery[0];
    releaseUploadedUrl();
    setActiveKind(kind);
    setError(null);
    setQuery(queryForSample(first));
    setResults(rankGallery(runtime, inputForSample(first), first.id));
  }

  function selectSample(sample: ShowcaseSample) {
    const runtime = runtimesRef.current?.[sample.kind];
    if (!runtime || queryBusy) {
      return;
    }

    releaseUploadedUrl();
    setError(null);
    setQuery(queryForSample(sample));
    setResults(rankGallery(runtime, inputForSample(sample), sample.id));
  }

  async function uploadQuery(file: File | undefined) {
    const runtime = runtimesRef.current?.[activeKind];
    if (!file || !runtime) {
      return;
    }

    setQueryBusy(true);
    setError(null);
    const mediaUrl = URL.createObjectURL(file);

    try {
      let input: SimilarityInput;
      if (activeKind === "image") {
        input = { bytes: new Uint8Array(await file.arrayBuffer()), kind: "image" };
      } else if (activeKind === "audio") {
        input = { features: await extractAudioSignature(file), kind: "features" };
      } else {
        input = { features: await extractVideoSignature(file), kind: "features" };
      }

      releaseUploadedUrl();
      uploadedUrlRef.current = mediaUrl;
      setQuery({
        id: null,
        kind: activeKind,
        label: file.name,
        mediaUrl: activeKind === "image" ? undefined : mediaUrl,
        previewUrl: activeKind === "image" ? mediaUrl : undefined,
      });
      setResults(rankGallery(runtime, input, null));
    } catch (cause) {
      URL.revokeObjectURL(mediaUrl);
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setQueryBusy(false);
    }
  }

  const mode = MODES[activeKind];
  const gallery = galleries[activeKind];

  return (
    <section className="space-y-6" aria-labelledby="live-demo-title">
      <div className="flex flex-col gap-5">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.18em] text-emerald-300">
            Live multimodal WASM demo
          </p>
          <h2 id="live-demo-title" className="mt-2 text-3xl font-semibold text-white">
            {mode.title}
          </h2>
          <p className="mt-2 max-w-3xl text-sm leading-6 text-neutral-400">{mode.description}</p>
        </div>

        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div
            className="inline-flex w-fit rounded-xl border border-neutral-800 bg-neutral-900 p-1"
            role="tablist"
            aria-label="Media type"
          >
            {(Object.keys(MODES) as MediaKind[]).map((kind) => (
              <button
                key={kind}
                type="button"
                role="tab"
                aria-selected={activeKind === kind}
                disabled={status !== "ready" || queryBusy}
                onClick={() => selectMode(kind)}
                className={`rounded-lg px-4 py-2 text-sm font-medium transition ${
                  activeKind === kind
                    ? "bg-emerald-400 text-neutral-950"
                    : "text-neutral-300 hover:bg-neutral-800 hover:text-white"
                } disabled:cursor-not-allowed disabled:opacity-50`}
              >
                {MODES[kind].label}
                {galleries[kind].length > 0 ? (
                  <span className="ml-2 opacity-60">{galleries[kind].length}</span>
                ) : null}
              </button>
            ))}
          </div>

          <label className="inline-flex cursor-pointer items-center justify-center rounded-xl border border-neutral-700 bg-neutral-900 px-4 py-2.5 text-sm font-medium text-neutral-100 hover:border-neutral-500">
            {queryBusy ? "Analysing locally…" : mode.uploadLabel}
            <input
              className="sr-only"
              type="file"
              accept={mode.accept}
              onChange={(event: ChangeEvent<HTMLInputElement>) => {
                const file = event.currentTarget.files?.[0];
                event.currentTarget.value = "";
                uploadQuery(file).catch((cause) => {
                  setError(cause instanceof Error ? cause.message : String(cause));
                });
              }}
              disabled={status !== "ready" || queryBusy}
            />
          </label>
        </div>
      </div>

      {status === "loading" ? (
        <div className="rounded-3xl border border-neutral-800 bg-neutral-900 p-8 text-neutral-300">
          Loading the Rust WebAssembly module and building three local indexes…
        </div>
      ) : null}

      {status === "error" ? (
        <div className="rounded-3xl border border-red-500/30 bg-red-500/10 p-6 text-sm text-red-200">
          The WASM demo could not start: {error}
        </div>
      ) : null}

      {status === "ready" ? (
        <div className="grid gap-6 lg:grid-cols-[0.8fr_1.2fr]">
          <div className="space-y-5 rounded-3xl border border-neutral-800 bg-neutral-900 p-5 sm:p-6">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-xs font-semibold uppercase tracking-[0.16em] text-neutral-500">
                  {mode.label} query
                </div>
                <div className="mt-1 font-medium text-white">{query?.label}</div>
              </div>
              <span className="rounded-full bg-emerald-400/10 px-3 py-1 text-xs font-medium text-emerald-300">
                local only
              </span>
            </div>
            {query ? <QueryMedia query={query} /> : null}
            <SamplePicker gallery={gallery} query={query} onSelect={selectSample} />
          </div>

          <div className="rounded-3xl border border-neutral-800 bg-neutral-900 p-5 sm:p-6">
            <div className="mb-5 flex items-center justify-between gap-4">
              <div>
                <div className="text-xs font-semibold uppercase tracking-[0.16em] text-neutral-500">
                  Ranked results
                </div>
                <div className="mt-1 text-sm text-neutral-300">{mode.metric}</div>
              </div>
              <span className="shrink-0 font-mono text-xs text-emerald-300">wasm32</span>
            </div>
            <RankedResults results={results} />
          </div>
        </div>
      ) : null}

      {error && status === "ready" ? (
        <p className="text-sm text-red-300">
          Could not analyse that {activeKind}. Browser codec support varies by file format: {error}
        </p>
      ) : null}

      <p className="max-w-4xl text-sm leading-6 text-neutral-400">
        The Pages path intentionally uses compact browser-safe signatures: pixels for images,
        spectral slices for audio, and sampled frames for video. The native service remains the
        owner of semantic embeddings, pHash, scene detection, speech/transcript analysis,
        recognized voices, Qdrant search, and corpus workflows.
      </p>
    </section>
  );
}

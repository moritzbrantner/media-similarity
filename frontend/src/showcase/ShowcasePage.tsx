import { useEffect, useRef, useState } from "react";
import { renderSampleGallery, type RenderedSample } from "./samples";
import {
  createSimilarityIndex,
  type SimilarityIndexClient,
  type SimilaritySearchHit,
} from "./wasm";

type RankedSample = {
  sample: RenderedSample;
  score: number;
};

type QueryPreview = {
  id: string | null;
  label: string;
  previewUrl: string;
};

const CAPABILITIES = [
  {
    title: "Image similarity",
    description: "Perceptual signatures, pHash near-duplicate detection, and vector search.",
  },
  {
    title: "Video and animation",
    description: "Scene-aware video indexing plus sampled, motion-aware GIF search.",
  },
  {
    title: "Audio",
    description: "Spectrogram search with speech, tempo, transcript, and voice metadata.",
  },
  {
    title: "Documents and text",
    description: "PDF page indexing, OCR/transcript filters, and searchable metadata.",
  },
  {
    title: "Identity",
    description: "Face and recognized-voice metadata for indexed media and smart albums.",
  },
  {
    title: "Corpus workflows",
    description: "Qdrant-backed indexing, saved smart albums, jobs, and source management.",
  },
];

function rankGallery(
  index: SimilarityIndexClient,
  gallery: RenderedSample[],
  queryBytes: Uint8Array,
  excludedId: string | null,
): RankedSample[] {
  return index
    .search(queryBytes, gallery.length)
    .filter((hit: SimilaritySearchHit) => gallery[hit.index]?.id !== excludedId)
    .map((hit) => ({ sample: gallery[hit.index], score: hit.score }))
    .filter((hit): hit is RankedSample => Boolean(hit.sample));
}

async function filePreview(file: File): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("Could not read image preview"));
    reader.readAsDataURL(file);
  });
}

export function ShowcasePage() {
  const indexRef = useRef<SimilarityIndexClient | null>(null);
  const [gallery, setGallery] = useState<RenderedSample[]>([]);
  const [query, setQuery] = useState<QueryPreview | null>(null);
  const [results, setResults] = useState<RankedSample[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function prepare() {
      try {
        const samples = await renderSampleGallery();
        const index = await createSimilarityIndex();
        samples.forEach((sample) => index.add(sample.bytes));

        if (cancelled) {
          index.free();
          return;
        }

        indexRef.current = index;
        setGallery(samples);
        setQuery({
          id: samples[0].id,
          label: samples[0].label,
          previewUrl: samples[0].previewUrl,
        });
        setResults(rankGallery(index, samples, samples[0].bytes, samples[0].id));
        setStatus("ready");
      } catch (cause) {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
          setStatus("error");
        }
      }
    }

    void prepare();
    return () => {
      cancelled = true;
      indexRef.current?.free();
      indexRef.current = null;
    };
  }, []);

  function selectSample(sample: RenderedSample) {
    const index = indexRef.current;
    if (!index) {
      return;
    }

    setError(null);
    setQuery({ id: sample.id, label: sample.label, previewUrl: sample.previewUrl });
    setResults(rankGallery(index, gallery, sample.bytes, sample.id));
  }

  async function uploadQuery(file: File | undefined) {
    const index = indexRef.current;
    if (!file || !index) {
      return;
    }

    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const previewUrl = await filePreview(file);
      setQuery({ id: null, label: file.name, previewUrl });
      setResults(rankGallery(index, gallery, bytes, null));
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <main className="min-h-screen bg-neutral-950 text-neutral-100">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-16 px-5 py-10 sm:px-8 lg:px-10 lg:py-16">
        <header className="grid gap-10 border-b border-neutral-800 pb-14 lg:grid-cols-[1.35fr_0.65fr] lg:items-end">
          <div className="space-y-6">
            <div className="inline-flex rounded-full border border-emerald-400/30 bg-emerald-400/10 px-3 py-1 text-xs font-semibold uppercase tracking-[0.18em] text-emerald-300">
              GitHub Pages · Rust → WebAssembly
            </div>
            <div className="space-y-4">
              <h1 className="max-w-4xl text-4xl font-semibold tracking-tight text-white sm:text-6xl">
                Search visually similar media directly in the browser.
              </h1>
              <p className="max-w-3xl text-base leading-7 text-neutral-300 sm:text-lg">
                This public showcase runs a compact image-similarity index in Rust WASM. No API,
                Qdrant instance, model download, or upload server is required for the live demo.
              </p>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-3 text-center text-sm">
            {[
              ["0", "server calls"],
              ["8", "demo images"],
              ["Rust", "search core"],
            ].map(([value, label]) => (
              <div key={label} className="rounded-2xl border border-neutral-800 bg-neutral-900 p-4">
                <div className="text-xl font-semibold text-white">{value}</div>
                <div className="mt-1 text-xs text-neutral-400">{label}</div>
              </div>
            ))}
          </div>
        </header>

        <section className="space-y-6" aria-labelledby="live-demo-title">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.18em] text-emerald-300">
                Live WASM demo
              </p>
              <h2 id="live-demo-title" className="mt-2 text-3xl font-semibold text-white">
                Pick an image. Rust ranks the gallery.
              </h2>
            </div>
            <label className="inline-flex cursor-pointer items-center justify-center rounded-xl border border-neutral-700 bg-neutral-900 px-4 py-2.5 text-sm font-medium text-neutral-100 hover:border-neutral-500">
              Try your own image
              <input
                className="sr-only"
                type="file"
                accept="image/png,image/jpeg,image/webp"
                onChange={(event) => void uploadQuery(event.currentTarget.files?.[0])}
                disabled={status !== "ready"}
              />
            </label>
          </div>

          {status === "loading" ? (
            <div className="rounded-3xl border border-neutral-800 bg-neutral-900 p-8 text-neutral-300">
              Loading the Rust WebAssembly module and building the in-browser index…
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
                      Query
                    </div>
                    <div className="mt-1 font-medium text-white">{query?.label}</div>
                  </div>
                  <span className="rounded-full bg-emerald-400/10 px-3 py-1 text-xs font-medium text-emerald-300">
                    local only
                  </span>
                </div>
                {query ? (
                  <img
                    src={query.previewUrl}
                    alt={query.label}
                    className="aspect-[8/5] w-full rounded-2xl object-cover"
                  />
                ) : null}
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-2">
                  {gallery.map((sample) => (
                    <button
                      type="button"
                      key={sample.id}
                      onClick={() => selectSample(sample)}
                      className={`overflow-hidden rounded-xl border text-left transition ${
                        query?.id === sample.id
                          ? "border-emerald-400 bg-emerald-400/10"
                          : "border-neutral-800 bg-neutral-950 hover:border-neutral-600"
                      }`}
                    >
                      <img
                        src={sample.previewUrl}
                        alt=""
                        className="aspect-[8/5] w-full object-cover"
                      />
                      <div className="px-3 py-2">
                        <div className="text-xs font-medium text-neutral-200">{sample.label}</div>
                        <div className="text-[11px] text-neutral-500">{sample.family}</div>
                      </div>
                    </button>
                  ))}
                </div>
              </div>

              <div className="rounded-3xl border border-neutral-800 bg-neutral-900 p-5 sm:p-6">
                <div className="mb-5 flex items-center justify-between">
                  <div>
                    <div className="text-xs font-semibold uppercase tracking-[0.16em] text-neutral-500">
                      Ranked results
                    </div>
                    <div className="mt-1 text-sm text-neutral-300">
                      12×12 RGB signature · mean absolute visual distance
                    </div>
                  </div>
                  <span className="font-mono text-xs text-emerald-300">wasm32</span>
                </div>
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
                        </div>
                        <div>
                          <div className="flex items-center justify-between text-xs">
                            <span className="text-neutral-500">similarity</span>
                            <span className="font-mono text-neutral-200">
                              {(score * 100).toFixed(1)}%
                            </span>
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
              </div>
            </div>
          ) : null}

          {error && status === "ready" ? (
            <p className="text-sm text-red-300">Could not search that image: {error}</p>
          ) : null}

          <p className="max-w-4xl text-sm leading-6 text-neutral-400">
            The live path intentionally demonstrates a lightweight perceptual image signature. The
            full native service remains the owner of semantic embeddings, pHash, Qdrant search,
            video/audio/PDF processing, identity metadata, and corpus workflows.
          </p>
        </section>

        <section className="space-y-6" aria-labelledby="capabilities-title">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-neutral-500">
              Native service
            </p>
            <h2 id="capabilities-title" className="mt-2 text-3xl font-semibold text-white">
              What media-similarity is built to do
            </h2>
          </div>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {CAPABILITIES.map((capability) => (
              <article
                key={capability.title}
                className="rounded-2xl border border-neutral-800 bg-neutral-900 p-5"
              >
                <h3 className="font-semibold text-white">{capability.title}</h3>
                <p className="mt-2 text-sm leading-6 text-neutral-400">{capability.description}</p>
              </article>
            ))}
          </div>
        </section>

        <footer className="flex flex-col gap-3 border-t border-neutral-800 pt-8 text-sm text-neutral-500 sm:flex-row sm:items-center sm:justify-between">
          <span>media-similarity · Rust, WASM, React, Qdrant</span>
          <a
            href="https://github.com/moritzbrantner/media-similarity"
            className="text-neutral-300 underline decoration-neutral-700 underline-offset-4 hover:text-white"
          >
            View source on GitHub
          </a>
        </footer>
      </div>
    </main>
  );
}

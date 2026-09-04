import { MediaDemo } from "./MediaDemo";

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

export function ShowcasePage() {
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
                Search similar images, audio, and video directly in the browser.
              </h1>
              <p className="max-w-3xl text-base leading-7 text-neutral-300 sm:text-lg">
                The public showcase extracts lightweight media signatures locally, then uses one
                deterministic Rust WASM ranker. No API, model download, Qdrant instance, or upload
                server is required.
              </p>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-3 text-center text-sm">
            {[
              ["0", "server calls"],
              ["3", "media types"],
              ["Rust", "WASM ranker"],
            ].map(([value, label]) => (
              <div key={label} className="rounded-2xl border border-neutral-800 bg-neutral-900 p-4">
                <div className="text-xl font-semibold text-white">{value}</div>
                <div className="mt-1 text-xs text-neutral-400">{label}</div>
              </div>
            ))}
          </div>
        </header>

        <MediaDemo />

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

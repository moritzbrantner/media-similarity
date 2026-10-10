# Dependency boundaries

`media-similarity` is an application and may compose audio, visual, text, model-runtime, and vector capabilities. That composition is intentional. The application must not make its domain model depend on those repositories' internal package topology.

## Layers

```text
backend/src/domain
        ↑
backend/src/app + storage + api
        ↑
backend/src/workers/media
        ↑
versioned external capability packages
```

`backend/src/domain` owns application data and similarity semantics. It must not import `audio-analysis`, `visual-analysis`, NLP transcript/model packages, or runtime/vector implementations.

`backend/src/workers/media` is the primary adapter boundary. Concrete audio, image, video, OCR, face, transcription, model, and embedding integrations belong there. An adapter may understand the public API of the capability it adapts; callers should consume application-owned results rather than importing that implementation crate themselves.

API handlers may orchestrate adapters, but new capability-specific conversion logic should move into `workers/media` rather than spreading crate imports through HTTP code.

## Source development

The committed `.coding-tooling.source-deps.json` intentionally contains no ambient source patches. Normal application development uses the versioned dependencies in `backend/Cargo.toml`.

A task that deliberately changes one external capability and this application may activate an exact source override for that task. Do not commit a standing graph of every audio/visual/NLP/foundation crate merely because the application can use those capabilities. Remove the override after the cross-repository migration is complete.

If a change needs several upstream repositories at exact HEAD simultaneously, treat it as an architecture/migration task and improve the public capability boundary before expanding the source graph.

## Guard

Run:

```bash
python3 scripts/check_dependency_boundaries.py
```

The guard blocks direct capability implementation imports from the domain layer. It treats every `moenarch-*` dependency declared in `backend/Cargo.toml` as a capability implementation (under whatever Rust name the dependency key gives it) unless the script's `DOMAIN_ALLOWED_PACKAGES` lists it as a platform primitive. It is intentionally about dependency direction, not a raw dependency-count limit.

## Declared repository graph

The `graph` in `.coding-tooling.dependencies.json` declares the intended capability topology (#44): each capability repository builds on the foundation only, and the generic `coding-tooling dependencies audit` checks this application against it. The released crates in `backend/Cargo.lock` currently deviate from that topology, and those deviations are upstream debt rather than part of the declaration:

- `moenarch-audio-analysis-core` and the other audio crates depend on `moenarch-video-analysis-core`; audio recognition/transcription depend on `moenarch-text-transcripts` and `moenarch-text-model-runtime` (audio → visual, audio → NLP).
- `moenarch-image-analysis-ocr` depends on `moenarch-text-core` (visual → NLP).
- `moenarch-text-transcripts` depends on `moenarch-audio-analysis-core` and `moenarch-video-analysis-ingest`, and several text crates on `moenarch-video-analysis-core` (NLP → audio, NLP → visual).
- Foundation crates such as `moenarch-math-signal-core`, `moenarch-numbers-core`, `moenarch-tensor-data` and `moenarch-vector-analysis-core` depend on `moenarch-video-analysis-core` (foundation → visual).

Re-check this list against `backend/Cargo.lock` when upstream releases change. Do not add these edges to the declared graph to make it match; remove them upstream.


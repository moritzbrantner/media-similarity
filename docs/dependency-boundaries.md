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

The guard blocks direct capability implementation imports from the domain layer. It is intentionally about dependency direction, not a raw dependency-count limit.

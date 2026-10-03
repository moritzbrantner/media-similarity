# Image Similarity Investigation

## Executive Summary

The observed bad face-photo matches are consistent with two separate issues in the current design:

- General media upload search uses the `visual` vector, which is a full-frame CLIP-style visual embedding, not a face-identity embedding.
- The visual embedding path can silently degrade to the legacy color-bucket embedder after model unavailability, initialization failure, runtime error, or timeout. That fallback is weak enough that unrelated portraits with similar crop, lighting, or background can score very high under cosine similarity.

The codebase already has a separate face pipeline using YuNet detection and OpenCV SFace embeddings. That pipeline writes/searches a separate Qdrant `face` vector, but ordinary media upload search does not route likely identity queries through it.

## Current Pipeline

Indexing for static images decodes media, analyzes faces, extracts OCR, builds payload, generates the visual embedding, and upserts the media point. The main static-image path is in `ImageIndexer::index_one`: media is loaded at [backend/src/workers/indexer/planning_methods_2.rs:160](../backend/src/workers/indexer/planning_methods_2.rs), face analysis runs at [backend/src/workers/indexer/planning_methods_2.rs:164](../backend/src/workers/indexer/planning_methods_2.rs), visual embedding is generated at [backend/src/workers/indexer/planning_methods_2.rs:192](../backend/src/workers/indexer/planning_methods_2.rs), and the media point is upserted at [backend/src/workers/indexer/planning_methods_2.rs:201](../backend/src/workers/indexer/planning_methods_2.rs).

Video scenes and audio segments follow the same visual-vector storage pattern at [backend/src/workers/indexer/planning_methods_2.rs:263](../backend/src/workers/indexer/planning_methods_2.rs) and [backend/src/workers/indexer/planning_methods_2.rs:308](../backend/src/workers/indexer/planning_methods_2.rs).

Payload construction stores the poster-frame pHash and the current embedder model name at [backend/src/workers/indexer/planning_methods_4.rs:134](../backend/src/workers/indexer/planning_methods_4.rs) and [backend/src/workers/indexer/planning_methods_4.rs:151](../backend/src/workers/indexer/planning_methods_4.rs). Static images use one sampled frame, GIFs sample frames evenly, and video scenes sample frames according to stride and max-frame settings. Static image sampling is at [backend/src/workers/media/image_io.rs:95](../backend/src/workers/media/image_io.rs), GIF sampling is at [backend/src/workers/media/image_io.rs:135](../backend/src/workers/media/image_io.rs), and video-scene sampling is at [backend/src/workers/media/video.rs:251](../backend/src/workers/media/video.rs).

Search computes query pHash, embeds the query sampled frames, searches Qdrant's `visual` vector, compares pHash distances, and computes relevance. The query pHash is calculated at [backend/src/workers/search.rs:93](../backend/src/workers/search.rs), query embedding at [backend/src/workers/search.rs:105](../backend/src/workers/search.rs), Qdrant visual search at [backend/src/workers/search.rs:108](../backend/src/workers/search.rs), hash distance at [backend/src/workers/search.rs:137](../backend/src/workers/search.rs), and relevance score at [backend/src/workers/search.rs:138](../backend/src/workers/search.rs).

Qdrant creates named `visual` and `face` vectors with cosine distance. The names and expected distance are defined at [backend/src/storage/qdrant/store.rs:14](../backend/src/storage/qdrant/store.rs), and the collection schema is created at [backend/src/storage/qdrant/operations_methods_1.rs:61](../backend/src/storage/qdrant/operations_methods_1.rs). Media upserts only the `visual` vector at [backend/src/storage/qdrant/operations_methods_1.rs:186](../backend/src/storage/qdrant/operations_methods_1.rs), while face upserts only the `face` vector at [backend/src/storage/qdrant/operations_methods_1.rs:210](../backend/src/storage/qdrant/operations_methods_1.rs). Search uses named vectors directly at [backend/src/storage/qdrant/operations_methods_2.rs:20](../backend/src/storage/qdrant/operations_methods_2.rs).

## Fallback Behavior

The visual embedder is built by `build_visual_embedder`. If `visual_embedding_enabled` is false or `visual_embedding_backend` is not `onnx`, it returns `LegacyColorEmbedder("legacy-disabled", ...)` at [backend/src/workers/media/visual_embedding.rs:335](../backend/src/workers/media/visual_embedding.rs).

When ONNX is enabled, `FallbackVisualEmbedder` wraps `OnnxVisualEmbedder` and `LegacyColorEmbedder` at [backend/src/workers/media/visual_embedding.rs:198](../backend/src/workers/media/visual_embedding.rs). It falls back when:

- The configured model bundle is unavailable and the legacy path-based model/preprocessor files are unavailable. Availability is checked at [backend/src/workers/media/visual_embedding.rs:126](../backend/src/workers/media/visual_embedding.rs).
- Runner initialization fails through `OnnxImageEmbedder::from_bundle` or path-based bundle construction at [backend/src/workers/media/visual_embedding.rs:151](../backend/src/workers/media/visual_embedding.rs).
- ONNX embedding returns an error at [backend/src/workers/media/visual_embedding.rs:191](../backend/src/workers/media/visual_embedding.rs).
- ONNX embedding times out after 30 seconds; the timeout constant is [backend/src/workers/media/visual_embedding.rs:16](../backend/src/workers/media/visual_embedding.rs), and the timeout wrapper is [backend/src/workers/media/visual_embedding.rs:312](../backend/src/workers/media/visual_embedding.rs).
- Any primary image/media failure sets `primary_disabled`, causing subsequent calls to use legacy. Image failure handling is at [backend/src/workers/media/visual_embedding.rs:284](../backend/src/workers/media/visual_embedding.rs), and media failure handling is at [backend/src/workers/media/visual_embedding.rs:295](../backend/src/workers/media/visual_embedding.rs).

This means one transient ONNX error can switch the process to degraded visual embeddings until restart.

## Instrumentation Gap

Fallback is only partially visible today.

Existing visibility:

- A warning log is emitted on first image fallback and media fallback at [backend/src/workers/media/visual_embedding.rs:289](../backend/src/workers/media/visual_embedding.rs) and [backend/src/workers/media/visual_embedding.rs:306](../backend/src/workers/media/visual_embedding.rs).
- Search responses include `query_visual_embedding_model` and `query_visual_embedding_degraded` at [backend/src/workers/search.rs:177](../backend/src/workers/search.rs).
- Indexed payloads include `visual_embedding_model` at [backend/src/workers/indexer/planning_methods_4.rs:151](../backend/src/workers/indexer/planning_methods_4.rs).
- Index jobs warn when model status is blocking at [backend/src/api/indexing.rs:77](../backend/src/api/indexing.rs).

Gaps:

- There is no metric or counter for per-index/per-query embedder path.
- There is no cardinality-safe tracing field consistently emitted with operation, model role, embedder implementation, and degraded status.
- Once `primary_disabled` is set, `embed_media` can return fallback output directly without emitting a warning for each degraded operation at [backend/src/workers/media/visual_embedding.rs:299](../backend/src/workers/media/visual_embedding.rs).
- Payload model names help after the fact, but only for indexed media and only if users inspect payloads.

## Legacy Embedder Behavior

The legacy embedder is not semantic. `ImageEmbedder::encode` allocates a configured-size vector, splits dimensions into RGB channel ranges, then adds each pixel channel value into a bucket chosen by `pixel_index % channel_range_len` at [backend/src/workers/media/embedder.rs:15](../backend/src/workers/media/embedder.rs). It L2-normalizes the vector at [backend/src/workers/media/embedder.rs:33](../backend/src/workers/media/embedder.rs).

For media with multiple frames, it averages per-frame vectors by frame delay and optionally blends in a motion signal computed from frame deltas at [backend/src/workers/media/embedder.rs:37](../backend/src/workers/media/embedder.rs). The vector is normalized again at [backend/src/workers/media/embedder.rs:62](../backend/src/workers/media/embedder.rs).

This scheme has no face detection, face crop, learned identity representation, object semantics, or spatial model beyond pixel index buckets. It is plausible for unrelated portraits to score near-collinear when they share dominant colors, lighting, crop ratio, clothing/background tones, or poster-frame composition.

## Model Choice And Query Intent

The configured visual model is a general image embedding model. Defaults set `clip_model_name` to `sentence-transformers/clip-ViT-B-32`, visual backend to `onnx`, and visual vector size to 512 at [backend/src/config/defaults.rs:128](../backend/src/config/defaults.rs). The active model bundle spec uses `ImageEmbeddingPreset::XenovaClipVitBasePatch32Onnx` at [backend/src/workers/media/models.rs:122](../backend/src/workers/media/models.rs).

The sibling image embedding crate identifies that preset as Xenova CLIP ViT-B/32 and selects `onnx/vision_model_quantized.onnx` or `onnx/vision_model.onnx` from `Xenova/clip-vit-base-patch32` in `../../rust-packages/t3code-0ed65866/crates/image/image-analysis-embeddings/src/lib.rs:68`.

CLIP-style image embeddings are useful for broad visual/contextual similarity, but they are not face-identity embeddings. They embed the full frame as loaded by `ImageSearchService`, with no face detection or crop in the ordinary media upload path at [backend/src/workers/search.rs:105](../backend/src/workers/search.rs).

## Face Pipeline State

The codebase already has a dedicated face pipeline. Face analysis detects faces and embeds each detected face while indexing at [backend/src/workers/media/faces.rs:76](../backend/src/workers/media/faces.rs). The analyzer only processes up to `face_max_frames_per_media` sampled frames at [backend/src/workers/media/faces.rs:220](../backend/src/workers/media/faces.rs), embeds each face with the detection at [backend/src/workers/media/faces.rs:230](../backend/src/workers/media/faces.rs), assigns a person cluster at [backend/src/workers/media/faces.rs:136](../backend/src/workers/media/faces.rs), and upserts the face vector at [backend/src/workers/media/faces.rs:159](../backend/src/workers/media/faces.rs).

Face search upload refuses to run when face detection or face embedding is inactive at [backend/src/api/search/face_upload.rs:30](../backend/src/api/search/face_upload.rs). It detects query faces, selects the largest/highest-confidence face, embeds it, and searches the `face` vector at [backend/src/api/search/face_upload.rs:49](../backend/src/api/search/face_upload.rs) and [backend/src/api/search/face_upload.rs:71](../backend/src/api/search/face_upload.rs).

Defaults enable face analysis and configure YuNet/SFace paths and 512-dimensional face embeddings at [backend/src/config/defaults.rs:191](../backend/src/config/defaults.rs). The model specs are YuNet and OpenCV SFace at [backend/src/workers/media/models.rs:126](../backend/src/workers/media/models.rs) and [backend/src/workers/media/models.rs:130](../backend/src/workers/media/models.rs).

The product issue is therefore routing and observability, not absence of all face-identity infrastructure.

## Normalization And Qdrant Distance

Qdrant uses cosine distance for both named vectors at [backend/src/storage/qdrant/store.rs:14](../backend/src/storage/qdrant/store.rs) and [backend/src/storage/qdrant/operations_methods_1.rs:64](../backend/src/storage/qdrant/operations_methods_1.rs).

Legacy visual vectors are L2-normalized in `ImageEmbedder::encode` and multi-frame media encoding at [backend/src/workers/media/embedder.rs:81](../backend/src/workers/media/embedder.rs). The trait-level multi-frame ONNX path also normalizes after weighted averaging at [backend/src/workers/media/visual_embedding.rs:57](../backend/src/workers/media/visual_embedding.rs).

The sibling crate's current `OnnxImageEmbedder` default also normalizes single-image outputs: `OnnxImageEmbeddingOptions::default` sets `normalize: true`, and `embed_image` calls `normalize_vector` before returning the embedding in `../../rust-packages/t3code-0ed65866/crates/image/image-analysis-embeddings/src/lib.rs:380` and `../../rust-packages/t3code-0ed65866/crates/image/image-analysis-embeddings/src/lib.rs:546`.

The service itself does not assert or record vector norms at storage/query boundaries, so a future dependency or config change could regress normalization without a local guard.

## Root Cause Findings

1. Same-person search is conflated with broad visual similarity in ordinary media upload search. That path uses full-frame `visual` embeddings and never invokes the dedicated face detector/embedder.
2. The visual embedding fallback can degrade to a color-bucket vector silently after the first failure. Warning logs exist, but there is no durable metric or consistent per-operation signal.
3. The legacy fallback embedder is not fit for identity or semantic visual ranking. It can plausibly generate very high cosine scores for unrelated people with similar color distributions.
4. Near-duplicate boosting is not the cause of the observed examples when pHash distances are 20-32. `relevance_score` only boosts when distance is at or under `duplicate_hash_distance`, default 8, at [backend/src/workers/search.rs:244](../backend/src/workers/search.rs).
5. The existing face vector and face upload endpoint indicate the likely direction: identity queries should use the `face` vector with detection/cropping, while contextual media search should use a trustworthy visual model.

## Out Of Scope But Noted

- Existing indexed media may contain legacy visual vectors if a previous process ran in degraded mode; there is no built-in stale-vector report keyed by embedder implementation.
- `primary_disabled` is process-local. Restarting may restore ONNX, but persisted legacy-indexed vectors remain until reindex.
- Face clustering threshold and minimum cluster settings may need their own quality evaluation after routing is fixed.
- The current quality corpus is portrait-heavy and identity-focused. Broader visual/contextual similarity needs a separate labeled corpus.

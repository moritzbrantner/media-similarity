# Image Similarity Improvement PRD

## Problem Statement

Media upload search currently mixes two different user intents: finding the same person across photos and finding broadly similar visual/contextual media. The current ordinary search path always embeds sampled full frames into the `visual` vector and ranks by Qdrant vector score with only a pHash near-duplicate boost.

The investigation found that this design can return unrelated people with very high scores because identity search is being served by a full-frame CLIP-style visual embedding, and because the visual embedder can silently degrade to a legacy color-bucket fallback. See [image-similarity-investigation.md](image-similarity-investigation.md).

The spike added a runnable diagnostic for comparing active visual, explicit legacy color, and face-identity embeddings on public and private image pairs. See [image-similarity-embedder-spike.md](image-similarity-embedder-spike.md).

## Goals

- Improve result quality for face-identity matching.
- Improve or preserve broader visual/contextual similarity search.
- Make degraded embedding paths observable during indexing and query operations.
- Ensure vector normalization behavior is explicit and covered by tests.
- Provide a migration path for any existing legacy or stale vectors.

## Non-Goals

- Do not tune acceptance criteria around private-only false-positive examples.
- Do not remove pHash near-duplicate detection.
- Do not require committing private image fixtures.
- Do not replace all model infrastructure in one step.

## Recommended Solution

Treat face identity and broad visual similarity as separate query intents.

For `face_identity` intent:

- Use the existing face detection and face embedding pipeline.
- Search Qdrant's existing `face` named vector.
- Select/crop the query face using the existing face upload behavior.
- Aggregate matching face points back to media results as the current face upload route already does.
- Return a clear degraded/unavailable response when face models are inactive, rather than falling back to visual search.

For `visual` intent:

- Continue to use the `visual` named vector for broad visual/contextual similarity.
- Keep CLIP-style visual embeddings unless the diagnostic and quality corpus show a better general model.
- Do not interpret high full-frame visual similarity as person identity.

For degraded visual behavior:

- Replace silent fallback with explicit degraded-mode policy.
- Preferred default: if visual embedding is enabled and ONNX is configured, fail indexing/query embedding loudly when ONNX is unavailable or errors.
- Optional operator override: allow explicit degraded mode for demos or low-quality local operation, but label every index/query operation as degraded.
- Keep legacy color embedding only as an explicit diagnostic or low-quality fallback, not as transparent production behavior.

For observability:

- Emit structured logs or metrics for every index/query embedding operation with cardinality-safe labels:
  - `operation=index|query`
  - `model_role=visual|face`
  - `embedder=onnx|legacy`
  - `degraded=true|false`
  - `media_kind=static_image|animated_gif|video_scene|audio|pdf_page`
- Add counters for fallback attempts, fallback successes, ONNX errors, and ONNX timeouts.
- Surface degraded status in API responses and model/status endpoints.

For normalization:

- Add local tests asserting vector norms for active visual, legacy visual, and face embedding paths where feasible.
- Document whether a model adapter is expected to return normalized vectors.
- Normalize at the service boundary if the adapter contract cannot be guaranteed.

## API And UI Implications

- Add or clarify query intent selection: `visual` and `face_identity`.
- Preserve existing media upload search behavior for default visual search unless the user selects identity intent.
- Route identity uploads to the existing face-search flow, including no-face-detected and model-inactive responses.
- Show model/degraded status in the UI near search results and model settings.
- Avoid presenting visual score as identity confidence.

## Qdrant And Migration

The desired schema already exists in new collections: named vectors `visual` and `face`, both cosine-distance. Rollout must still verify deployed collections match expected vector names, dimensions, and distances.

Migration steps:

1. Add a schema/status check that reports deployed `visual` and `face` vector dimensions.
2. Add an indexed-vector audit that identifies media payloads with legacy visual model names or missing face vectors.
3. If visual model or dimension changes, create a new collection or migration path rather than mixing incompatible vector spaces.
4. Re-embed media `visual` vectors and face `face` vectors separately.
5. Track reindex completion, stale vector counts, and failed media IDs.

## Ranking

- Keep the pHash near-duplicate boost for near-duplicate detection.
- Do not use pHash distance to rescue identity search.
- For visual search, keep `relevance_score` mostly aligned with vector score once vector quality is trustworthy.
- For face identity search, rank primarily by face vector score and aggregate by person/media; do not blend with full-frame visual score unless a later quality evaluation proves it helps.

## Success Metrics

- Face identity precision@k on the public quality corpus.
- Different-person false-positive rate on labeled negative pairs.
- Visual/contextual precision@k on a separate non-identity quality corpus.
- Embedder fallback/degraded rate visible in logs or dashboards.
- Count of stale indexed media vectors by model name.
- Count of media with expected face vectors missing after reindex.
- Median and p95 query latency for visual and face identity searches.

## Rollout

1. Add observability for current visual and face embedding paths.
2. Run the diagnostic and quality corpus locally with public fixtures.
3. Add optional private local pair manifests for observed false positives.
4. Shadow-score new face identity results beside current visual results without changing default behavior.
5. Backfill/reindex face vectors and any stale visual vectors.
6. Enable explicit `face_identity` search intent in API/UI.
7. Cut over identity UI flows to face search after quality metrics pass.
8. Disable transparent legacy fallback by default.

## Open Questions And Risks

- Running two embedding models may increase CPU, memory, and latency costs.
- Face model download/cache state may block identity search in environments that currently rely on visual-only search.
- Full-corpus reindex cost may be high for large video libraries.
- Production may already contain legacy visual vectors with no easy metric to estimate prevalence until an audit is added.
- Face detection can miss profile faces, occlusions, low-resolution faces, or stylized media.
- A separate broad visual/contextual corpus is needed to avoid over-optimizing for portraits.

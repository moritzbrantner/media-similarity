# Optional derived metadata index

Status: accepted implementation of #89; the existing provider remains the default.

Qdrant owns media payloads and vectors. `METADATA_FILTER_PROVIDER=search-engine`
selects a server-local, rebuildable Tantivy projection through the public
`search-engine` facade, pinned to an immutable public Git revision. It narrows
Qdrant candidates by physical point ID; Qdrant still computes vector scores.
Application scoring, duplicate detection, authorization/source filtering and
smart-album criteria retain their existing owners. Tantivy scores never become
visual or OCR relevance scores.

`METADATA_INDEX_DIR` defaults to `data/metadata-index`. A collection-name hash
isolates its projection. Use one writer process per directory and collection;
do not point independent application instances at the same directory. The first
collection initialization or filtered request reconciles the persisted projection against all current Qdrant
payloads. Successful application writes/deletes update the projection; ambiguous
writes invalidate it and the next filtered read reconciles again. Projection
revisions include deletion tombstones, so removal and reinsertion remain ordered
across restarts. Deleting this derived directory while the service is stopped
rebuilds from Qdrant on the next filtered request. External Qdrant writers require
a service restart to reconcile; this optional mode assumes application-owned
mutations between restarts.

Shared predicates cover case-insensitive name/path and camera/lens substrings,
source and media kind, dimensions, byte size, optional capture time, modification
time, GPS presence, and relevant OCR/transcript text. Capture times use the same
parser as Qdrant and remain absent when missing. Text candidates are a superset;
the application retains final text matching and relevance. MIME is not a canonical
payload field and has no new filter. Person, speaker, keyword, orientation and
duplicate criteria retain existing application filtering. No original media,
vectors, or new corpus schema is stored in Tantivy.

This is a consumer experiment, not approval to switch the production default.
Startup, first-filter reconciliation, query time and process-memory observations
must be interpreted with their fixture size, build profile and environment.
The API acceptance test compares IDs and all existing score fields through
updates, deletion, restart and rebuild; the opt-in real-Qdrant variant is:

```bash
QDRANT_URL=http://127.0.0.1:6333 COMPOSE_SERVICES=qdrant \
  bash scripts/run-with-compose-services.sh cargo test \
  --manifest-path backend/Cargo.toml --locked --test metadata_provider_api \
  metadata_providers_match_real_qdrant -- --ignored --nocapture
```

Set `METADATA_ACCEPTANCE_BROWSER=1` on that command to exercise the checked-in
frontend with Chromium against both native services through the same lifecycle.
The `qdrant_smoke` test name also includes this real-service test in existing CI.

## Local acceptance observations

At the implementation plus current dependency merge (`9ddc915`), all backend
tests and lint passed. The real-Qdrant lifecycle test passed, including six
unmocked Chromium sessions: initial and updated results for both providers, then
selected-provider restart and rebuild. IDs, visual scores, OCR scores, relevance
scores and pHash distances matched in the native API comparisons. Models and OCR
inference were disabled; this verifies metadata routing and existing visual
similarity with the legacy embedder, not model quality.

A separate native debug-binary probe used Qdrant 1.19.2 on loopback, 100 local
80×40 JPEG paths, the legacy 32-dimensional embedder, no background watching or
models, and a name filter matching all 100 rows. Three fresh service processes
per provider each performed one first query and ten warm queries. Median values:

| Observation | existing | search-engine |
| --- | ---: | ---: |
| Process spawn to HTTP health | 32.8 ms | 33.0 ms |
| First filtered upload query | 21.6 ms | 49.8 ms |
| Warm upload query (median of per-process medians) | 18.6 ms | 23.0 ms |
| Process RSS after queries | 47,228 KiB | 52,760 KiB |

The very first derived-index creation took 84.9 ms within its first query;
subsequent fresh processes reconciled the persisted index in their first query.
RSS is sampled from `/proc/<pid>/status`, not peak memory. This tiny, sequential,
warm-filesystem debug probe establishes overhead only; it makes no scale or
production speed claim. It supports retaining the existing default. Disposable
collections, service processes, Qdrant containers and networks were cleaned up.

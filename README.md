# Image Similarity Service

Native Rust image similarity search service with a React UI and Qdrant vector storage.

The service indexes configured image folders, generates thumbnails and perceptual hashes, then lets you upload a query image or video through the web UI or HTTP API to find visually similar images and near duplicates. Animated GIFs are supported with sampled frame and motion-aware vector search. Uploaded videos are detected and split into scenes with the sibling Rust `video-analysis` crates, then each scene is searched independently.

## Features

- Rust backend using Axum for health, indexing, search, thumbnail, and static UI routes.
- Local folder indexing with deterministic image IDs.
- Qdrant REST integration for vector upsert and search.
- Native image loading for JPEG, PNG, GIF, WebP, BMP, and TIFF.
- Uploaded video query support for MP4, MOV, M4V, WebM, MKV, and AVI when `ffmpeg`/`ffprobe` are available.
- Local source video indexing: videos in configured local source folders are cut into scenes and indexed as individual searchable scene records.
- Scene detection and scene splitting through the Rust `video-analysis-core`, `video-analysis-detectors`, `video-analysis-ffmpeg`, and `video-analysis-split` crates.
- Native pHash generation, pHash Hamming distance, and thumbnail generation.
- Animation-aware GIF indexing and upload search using sampled frame content plus motion deltas.
- Deterministic normalized image-vector embedder in Rust.
- React UI built with Bun, TypeScript, React Query, Tailwind CSS, and oxfmt.
- Docker Compose setup with the Rust app, Qdrant, and a sample people image seed job.

MinIO, `video://`, and camera source URI parsing is retained, but those source backends are currently reported as unavailable by the Rust service. Local video files inside configured local source folders are indexed as scene records.

## Quick Start

1. Copy the environment file:

   ```bash
   cp .env.example .env
   ```

2. Optionally edit `.env` and set `HOST_SOURCE_IMAGE_DIR` to your local image folder:

   ```env
   HOST_SOURCE_IMAGE_DIR=/absolute/path/to/your/images
   SOURCE_IMAGE_DIR=/images
   ```

   `HOST_SOURCE_IMAGE_DIR` is used by Docker Compose on the host. `SOURCE_IMAGE_DIR` is the path inside the app container and can usually stay `/images`.
   If you keep the default `./sample-images`, Docker Compose runs the `seed-data` job first and fills that directory with example people images from `https://thispersondoesnotexist.com/`.

3. Start the service:

   ```bash
   docker compose up --build
   ```

4. Open the UI:

   ```txt
   http://localhost:8000
   ```

5. Click **Index configured sources**, then upload a query image and search.

## API

### Health

```bash
curl http://localhost:8000/api/health
```

### Index Configured Sources

```bash
curl -X POST http://localhost:8000/api/index
```

### Search With Uploaded Image Or Video

```bash
curl -X POST "http://localhost:8000/api/search?limit=12" \
  -F "file=@/path/to/query.jpg"
```

The response includes:

- `vector_score`: Qdrant similarity score from vector search.
- `hash_distance`: pHash Hamming distance from the query image.
- `near_duplicate`: `true` when `hash_distance <= DUPLICATE_HASH_DISTANCE`.
- `thumbnail_url`: URL for the generated thumbnail served by the backend.
- `animated_thumbnail_url`: URL for generated animated GIF previews when the result is an animated GIF.
- `query_media_kind`: `static_image`, `animated_gif`, or `video`.
- `scenes`: per-scene search groups for video uploads, including scene frame/time bounds and a `clip_url` for the generated scene MP4.
- Matched source video scenes include `full_video_url`, `scene_clip_url`, and `scene_start_seconds`/`scene_end_seconds` so clients can open the source video at the matching time window.

GIF vector search uses sampled frame content plus frame-to-frame motion deltas. `query_phash`, `hash_distance`, and `near_duplicate` remain based on the representative poster frame so the duplicate contract stays compatible with static images.

Video query search and source video indexing use the Rust scene detection crates with the content detector defaults from the `vanalyze` CLI. The service writes per-scene MP4 clips under `UPLOAD_DIR`, samples scene frames according to `VIDEO_FRAME_STRIDE` and `VIDEO_MAX_FRAMES`, and searches/indexes each scene independently. The Rust crates are sufficient for this workflow, but their command-backed FFmpeg runtime requires `ffmpeg` and `ffprobe` on `PATH`.

## Configuration

Set these values in `.env`:

| Variable | Default | Purpose |
| --- | --- | --- |
| `HOST_SOURCE_IMAGE_DIR` | `./sample-images` | Host folder mounted into the app container. |
| `SOURCE_IMAGE_DIR` | `/images` | Container path scanned by the indexer. |
| `IMAGE_SOURCES` | empty | Optional source list. When empty, the indexer scans `SOURCE_IMAGE_DIR`. Use a JSON array, comma-separated list, semicolon-separated list, or newline-separated list. |
| `QDRANT_URL` | `http://qdrant:6333` | Qdrant URL from inside the app container. |
| `QDRANT_COLLECTION` | `image_similarity` | Qdrant collection name. |
| `VECTOR_SIZE` | `512` | Qdrant vector size for the Rust embedder. |
| `CLIP_MODEL_NAME` | `sentence-transformers/clip-ViT-B-32` | Kept for configuration compatibility; native Rust inference is not CLIP-equivalent yet. |
| `THUMBNAIL_DIR` | `/app/data/thumbnails` | Generated thumbnail storage. |
| `UPLOAD_DIR` | `/app/data/uploads` | Reserved local upload storage path. |
| `IMAGE_EXTENSIONS` | `.jpg,.jpeg,.png,.webp,.bmp,.tif,.tiff,.gif` | File extensions to index. |
| `DEFAULT_SEARCH_LIMIT` | `12` | Default result count. |
| `DUPLICATE_HASH_DISTANCE` | `8` | Max pHash distance for near-duplicate flag. |
| `MAX_UPLOAD_MB` | `20` | Maximum uploaded query image or video size. |
| `VIDEO_FRAME_STRIDE` | `30` | Frame stride used when sampling uploaded video scenes for search. |
| `VIDEO_MAX_FRAMES` | empty | Optional maximum sampled frames per uploaded video scene. Falls back to `GIF_SAMPLE_FRAMES` when unset. |
| `GIF_SAMPLE_FRAMES` | `16` | Maximum sampled GIF frames used for vector generation. |
| `GIF_MAX_DECODE_FRAMES` | `512` | Maximum GIF frames decoded before deterministic truncation. |
| `GIF_PREVIEW_FRAMES` | `16` | Maximum frames written to generated animated GIF previews. |
| `GIF_DEFAULT_FRAME_DELAY_MS` | `100` | Delay used when a GIF frame delay is zero or missing. |
| `GIF_MOTION_WEIGHT` | `0.2` | Blend weight for motion deltas in animation-aware GIF vectors. |
| `SAMPLE_FACE_DATA_DIR` | `/seed/local` | Container path where the seed job writes local sample images. |
| `SAMPLE_FACE_COUNT` | `150` | Number of example people images to download. |
| `SAMPLE_FACE_URL` | `https://thispersondoesnotexist.com/` | JPEG source used by the seed job. |
| `SAMPLE_FACE_DELAY_MS` | `1000` | Delay between download attempts, to avoid duplicate cached responses. |
| `SAMPLE_FACE_MAX_ATTEMPTS` | `750` | Maximum attempts allowed while collecting unique images. |
| `SAMPLE_FACE_CLEAR_GENERATED` | `true` | Remove prior generated `person-*.jpg` and legacy `dummy-*` files before seeding. |

## Source Examples

`IMAGE_SOURCES` accepts multiple local source paths. Local folder support is backward compatible with `SOURCE_IMAGE_DIR`.

```env
IMAGE_SOURCES='["local:///images", "/more-images"]'
```

Unsupported source types are surfaced in the index response as skipped sources instead of crashing the index run.

## Local Development

Run the backend locally:

```bash
QDRANT_URL=http://localhost:6333 \
SOURCE_IMAGE_DIR=/absolute/path/to/images \
cargo run --manifest-path rust/Cargo.toml --bin image-similarity-service
```

Run the Rust test suite:

```bash
cargo test --manifest-path rust/Cargo.toml
```

Build the runtime image:

```bash
docker build --build-context rust-packages=../rust-packages -t image-similarity-service .
```

Run the same Rust tests inside Docker Compose:

```bash
docker compose --profile test run --rm test
```

Run Qdrant only:

```bash
docker compose up qdrant
```

Download or refresh local sample people images without starting the full stack:

```bash
cargo run --manifest-path rust/Cargo.toml --bin seed_dummy_data
```

Run the Compose seed job again:

```bash
docker compose run --rm seed-data
```

### Frontend Development

Install frontend dependencies with Bun:

```bash
bun install
```

Run the React dev server:

```bash
bun run dev
```

The Vite dev server proxies `/api` and `/thumbnails` to `http://127.0.0.1:8000`, so run the Rust service separately while developing the UI.

Build the frontend into the backend static directory:

```bash
bun run build
```

Format and type-check the frontend:

```bash
bun run format
bun run typecheck
```

## Notes

- Re-running indexing upserts local images by deterministic ID based on absolute image path.
- If an image file changes at the same path, run indexing again to refresh its vector, pHash, metadata, and thumbnail.
- The Dockerfile uses Docker BuildKit's named `rust-packages` context for the sibling workspace required by the Cargo path dependencies.
- This version intentionally omits auth, users, async queues, and a separate frontend build pipeline.

# Image Similarity Service

Native Rust image similarity search service with a React UI and Qdrant vector storage.

The service indexes configured media folders, generates thumbnails and perceptual hashes, then lets you upload a query image, video, audio file, or PDF through the web UI or HTTP API to find visually similar media and near duplicates. Animated GIFs are supported with sampled frame and motion-aware vector search. Uploaded videos are detected and split into scenes with the sibling Rust `video-analysis` crates, then each scene is searched independently. Audio files are rendered to spectrogram images for indexing and search. PDFs are rendered page-by-page and indexed both as page records and whole-document summary records.

The repository is organized as a Rust-only backend plus a conventional React/Vite/Bun/Tailwind frontend. Python support and PyO3 extension packaging have been removed. Worker boundaries are Rust modules inside the single backend process.

## Features

- Rust backend using Axum for health, indexing, search, thumbnail, and static UI routes.
- Rust worker modules for indexing, source loading, media decoding/analysis, OCR, face/voice handling, thumbnails, and embeddings.
- Local folder indexing with deterministic media IDs.
- Qdrant REST integration for vector upsert and search.
- Model status and download jobs for visual, face, and audio transcription model roles through the sibling Rust model crates.
- Explicit index deletion APIs for removing indexed records and generated thumbnails/uploads without deleting original source files.
- Native image loading for JPEG, PNG, GIF, WebP, BMP, and TIFF.
- Uploaded video query support for MP4, MOV, M4V, WebM, MKV, and AVI when `ffmpeg`/`ffprobe` are available.
- Local source video indexing: videos in configured local source folders are cut into scenes and indexed as individual searchable scene records.
- Uploaded and local source audio support for MP3, WAV, FLAC, M4A, AAC, OGG, and Opus when `ffmpeg`/`ffprobe` are available.
- Uploaded and local source PDF support when Poppler `pdfinfo`, `pdftoppm`, and `pdftotext` are available.
- Audio speech-activity, tempo, bit-boundary, and recognized-voice metadata for indexed and uploaded audio.
- Scene detection and scene splitting through the Rust `video-analysis-core`, `video-analysis-detectors`, `video-analysis-ffmpeg`, and `video-analysis-split` crates.
- Native pHash generation, pHash Hamming distance, and thumbnail generation.
- Animation-aware GIF indexing and upload search using sampled frame content plus motion deltas.
- Deterministic normalized image-vector embedder in Rust.
- React UI built with Bun, TypeScript, React Query, Tailwind CSS, and oxfmt.
- Docker Compose setup with the Rust app, Qdrant, and default host media folder mounts.

MinIO/S3 object-store sources are supported through `minio://bucket/prefix` and `s3://bucket/prefix` source specs. `video://` and camera source URI parsing is retained, but those source backends are currently reported as unavailable by the Rust service. Local video files inside configured local source folders are indexed as scene records, and local audio files are indexed as spectrogram records.

## Quick Start

1. Copy the environment file:

   ```bash
   cp .env.example .env
   ```

2. Optionally edit `.env` to point the default media mounts at your local folders:

   ```env
   HOST_PICTURES_DIR=/absolute/path/to/your/pictures
   HOST_VIDEO_DIR=/absolute/path/to/your/videos
   HOST_AUDIO_DIR=/absolute/path/to/your/audio
   ```

   Docker Compose mounts those host folders read-only at `/media/pictures`, `/media/videos`, and `/media/audio`. The checked-in `config/media-sources.txt` file lists those container paths, one source per line.

3. Install frontend dependencies:

   ```bash
   bun install
   ```

4. Start the service:

   ```bash
   bun dev
   ```

5. Open the UI:

   ```txt
   http://localhost:5173
   ```

   `bun dev` starts the Docker Compose app stack in the background, then starts the Vite dev server. The backend container remains available at `http://localhost:8000`.

6. Click **Index configured sources**, then upload a query image, video, audio file, or PDF and search.

## Sample Corpus And Showcase Data

The repository includes a manifest-driven sample corpus for tests and demos. The
manifest lives at `tests/fixtures/sample-corpus/manifest.json`; downloaded media
is written to the ignored `sample-images/showcase` directory so large public
media does not churn the repository.

Validate the corpus contract:

```bash
bun run sample:check
```

Download the showcase corpus:

```bash
bun run sample:download
```

The downloader fetches popular public sample media:

- Wikimedia Commons `Example.jpg` for static image search.
- Wikimedia Commons rotating Earth GIF for animation-aware GIF search.
- Wikimedia Commons `Example.ogg` for audio search.
- Blender Foundation Big Buck Bunny MP4 for video scene search.
- W3C WAI dummy PDF for PDF page and document search.

It also creates exact-match query copies under `sample-images/showcase/queries`
and writes `sample-images/showcase/ATTRIBUTION.md` with source pages, license
labels, and attribution text.

To run the sample-corpus tests:

```bash
bun run test:sample
```

By default, the end-to-end showcase test looks for
`sample-images/showcase`. To test a corpus generated elsewhere, set
`SAMPLE_CORPUS_DIR`:

```bash
SAMPLE_CORPUS_DIR=/tmp/image-sim-sample-corpus bun run test:sample
```

To try the corpus in the UI, set a local source to the generated source folder,
start the app, index, then upload matching files from the generated query
folder:

```env
HOST_PICTURES_DIR=./sample-images/showcase/sources
HOST_VIDEO_DIR=./sample-images/showcase/sources
HOST_AUDIO_DIR=./sample-images/showcase/sources
```

For Docker Compose, use absolute host paths in `.env` if relative paths do not
mount as expected.

## API

### Health

```bash
curl http://localhost:8000/api/health
curl http://localhost:8000/api/ready
```

`/api/health` is a cheap liveness check. `/api/ready` verifies operational
dependencies such as Qdrant, writable data directories, configured sources, and
optional media tools.

### Index Configured Sources

```bash
curl -X POST http://localhost:8000/api/index
```

### Search With Uploaded Image, Video, Audio, Or PDF

```bash
curl -X POST "http://localhost:8000/api/search?limit=12" \
  -F "file=@/path/to/query.jpg"
```

The response includes:

- `vector_score`: Qdrant similarity score from vector search.
- `hash_distance`: pHash Hamming distance from the query media poster frame.
- `near_duplicate`: `true` when `hash_distance <= DUPLICATE_HASH_DISTANCE`.
- `thumbnail_url`: URL for the generated thumbnail served by the backend.
- `animated_thumbnail_url`: URL for generated animated GIF previews when the result is an animated GIF.
- `query_media_kind`: `static_image`, `animated_gif`, `video`, `audio`, or `pdf`.
- `scenes`: per-scene search groups for video uploads and per-bit search groups for audio uploads, including time bounds. Video scenes also include frame bounds and a `clip_url` for the generated scene MP4.
- PDF upload responses use `scenes` with `scene_kind: "pdf_page"` and page metadata so each query page can be inspected independently.
- Matched source video scenes include `full_video_url`, `scene_clip_url`, and `scene_start_seconds`/`scene_end_seconds` so clients can open the source video at the matching time window.
- Matched source audio records include `full_audio_url` and `scene_start_seconds`/`scene_end_seconds` so clients can play the matched audio bit.
- Matched source audio records include `audio_analysis` with `speech_detected`, `speech_ratio`, `speech_segments`, `audio_segments`, `recognized_voices`, `tempo_bpm`, `tempo_confidence`, and `tempo_onset_count`.
- Audio upload responses include `query_audio_analysis` with the same analysis shape for the query audio.

Search accepts server-side metadata filters as query parameters:

- `source_type`, `media_kind`, `name_query`, `camera_query`, `keyword_query`, and `person_id`.
- `has_gps=all|yes|no`, `near_duplicate=all|only|exclude`, and `orientation=all|landscape|portrait|square`.
- `min_width`, `max_width`, `min_height`, `max_height`, `min_size_bytes`, `max_size_bytes`.
- `modified_from`, `modified_to`, `captured_from`, and `captured_to` as Unix timestamps in seconds.

GIF vector search uses sampled frame content plus frame-to-frame motion deltas. `query_phash`, `hash_distance`, and `near_duplicate` remain based on the representative poster frame so the duplicate contract stays compatible with static images.

### Model Status

```bash
curl http://localhost:8000/api/models
curl -X POST http://localhost:8000/api/models/visual_embedding/download \
  -H 'Content-Type: application/json' \
  -d '{"model": null}'
```

Model roles are `visual_embedding`, `face_detection`, `face_embedding`, and `audio_transcription`. The service delegates model specs, bundle storage, and native runtime adapters to the sibling `../rust-packages` crates.

### Delete From Index

```bash
curl -X DELETE http://localhost:8000/api/indexed-media/<media-id>
curl -X DELETE 'http://localhost:8000/api/indexed-sources?source_uri=/media/pictures'
```

Deletion removes Qdrant media/face points and generated files under `THUMBNAIL_DIR` and `UPLOAD_DIR`. It does not delete original source files.

Video query search and source video indexing use the Rust scene detection crates with the content detector defaults from the `vanalyze` CLI. The service writes per-scene MP4 clips under `UPLOAD_DIR`, samples scene frames according to `VIDEO_FRAME_STRIDE` and `VIDEO_MAX_FRAMES`, and searches/indexes each scene independently. The Rust crates are sufficient for this workflow, but their command-backed FFmpeg runtime requires `ffmpeg` and `ffprobe` on `PATH`.

Audio query search and source audio indexing use FFmpeg to render deterministic spectrogram images, then reuse the same thumbnail, pHash, and vector search pipeline as image media. Audio duration metadata is read with `ffprobe`. Speech activity uses a deterministic RMS voice-activity detector, tempo uses onset detection plus BPM estimation over mono 16 kHz PCM extracted with FFmpeg, and audio bit boundaries are guessed from speech spans, speaker labels, onsets, and maximum bit duration. Recognized voices are stored in a persistent spectral speaker registry; this is a deterministic baseline suitable for matching recurring voices, not a biometric identity guarantee.

Audio transcript fields are retained in the API shape, but native whisper.cpp transcription is disabled by default and not bundled into the repository-local text compatibility layer.

PDF query search and source PDF indexing use Poppler commands. Each source PDF creates one `pdf_document` summary record plus one `pdf_page` record for each rendered page up to `PDF_MAX_PAGES`. PDF text search combines embedded text from `pdftotext` with OCR over rendered pages. The rendered page images reuse the same thumbnail, pHash, and vector search pipeline as other visual media.

## Configuration

Set these values in `.env`:

| Variable | Default | Purpose |
| --- | --- | --- |
| `HOST_PICTURES_DIR` | `${HOME}/Pictures` | Host pictures folder mounted into the app container. |
| `HOST_VIDEO_DIR` | `${HOME}/Videos` | Host video folder mounted into the app container. |
| `HOST_AUDIO_DIR` | `${HOME}/Music` | Host audio folder mounted into the app container. |
| `MEDIA_SOURCES_FILE` | `/app/data/media-sources.txt` | Writable source list file read by the Rust service when `IMAGE_SOURCES` is empty. In Docker this persists UI edits in the app data volume. |
| `MEDIA_SOURCES_SEED_FILE` | `/app/config/media-sources.txt` | Optional read-only seed source list used when `MEDIA_SOURCES_FILE` does not exist. |
| `SOURCE_IMAGE_DIR` | `/images` | Legacy fallback path scanned only when `IMAGE_SOURCES` is empty and no media sources file is available. |
| `IMAGE_SOURCES` | empty | Optional source list override. When set, this takes precedence over `MEDIA_SOURCES_FILE`. Use a JSON array, comma-separated list, semicolon-separated list, or newline-separated list. |
| `MINIO_ENDPOINT` | empty | MinIO/S3-compatible endpoint for `minio://` sources. Include a scheme or pair with `MINIO_SECURE`. |
| `MINIO_ACCESS_KEY` | empty | Access key for `minio://` sources. |
| `MINIO_SECRET_KEY` | empty | Secret key for `minio://` sources. |
| `MINIO_SECURE` | `true` | Use HTTPS for MinIO endpoints without an explicit scheme. |
| `S3_ENDPOINT` | empty | Optional custom endpoint for `s3://` sources. Leave empty for AWS S3 defaults. |
| `S3_ACCESS_KEY_ID` | empty | Optional access key for `s3://` sources. AWS environment credentials remain supported by the object-store client. |
| `S3_SECRET_ACCESS_KEY` | empty | Optional secret key for `s3://` sources. |
| `S3_REGION` | `us-east-1` | Region for `s3://` sources. |
| `S3_ALLOW_HTTP` | `false` | Allow HTTP custom S3 endpoints. |
| `QDRANT_URL` | `http://qdrant:6333` | Qdrant URL from inside the app container. |
| `QDRANT_COLLECTION` | `image_similarity` | Qdrant collection name. |
| `QDRANT_REQUEST_TIMEOUT_MS` | `30000` | Total timeout for each Qdrant HTTP request. |
| `QDRANT_CONNECT_TIMEOUT_MS` | `2000` | Timeout for establishing a Qdrant HTTP connection. |
| `QDRANT_RETRY_ATTEMPTS` | `2` | Additional retry attempts for transient Qdrant HTTP failures. |
| `QDRANT_RETRY_BACKOFF_MS` | `100` | Initial retry backoff for transient Qdrant HTTP failures. |
| `VECTOR_SIZE` | `512` | Qdrant vector size for the Rust embedder. |
| `CLIP_MODEL_NAME` | `sentence-transformers/clip-ViT-B-32` | Kept for configuration compatibility; native Rust inference is not CLIP-equivalent yet. |
| `THUMBNAIL_DIR` | `/app/data/thumbnails` | Generated thumbnail storage. |
| `UPLOAD_DIR` | `/app/data/uploads` | Reserved local upload storage path. |
| `IMAGE_EXTENSIONS` | `.jpg,.jpeg,.png,.webp,.bmp,.tif,.tiff,.gif` | File extensions to index. |
| `AUDIO_EXTENSIONS` | `.mp3,.wav,.flac,.m4a,.aac,.ogg,.opus` | Audio file extensions to index. |
| `PDF_EXTENSIONS` | `.pdf` | PDF file extensions to index. |
| `PDF_RENDER_DPI` | `144` | DPI used when rendering PDF pages with Poppler. |
| `PDF_MAX_PAGES` | `100` | Maximum pages indexed per PDF. |
| `PDF_SUMMARY_PAGES` | `8` | Maximum rendered pages sampled into the document summary vector. |
| `AUDIO_TRANSCRIPTION_ENABLED` | `false` | Compatibility switch for transcript analysis. The repository-local text compatibility layer does not bundle a native transcription backend. |
| `VOICE_REGISTRY_PATH` | `/app/data/recognized-voices.json` | Persistent speaker registry used to recognize recurring voices across audio files. |
| `MODEL_BUNDLE_DIR` | `/app/data/models/bundles` | Local model bundle directory used by rust-packages model stores. |
| `MODEL_HF_CACHE_DIR` | `/app/data/models/hf-cache` | Optional Hugging Face cache directory for model downloads. |
| `MODEL_HF_TOKEN` | empty | Optional Hugging Face token forwarded to model downloads. |
| `DEFAULT_SEARCH_LIMIT` | `12` | Default result count. |
| `DUPLICATE_HASH_DISTANCE` | `8` | Max pHash distance for near-duplicate flag. |
| `MAX_UPLOAD_MB` | `20` | Maximum uploaded query image, video, or audio size. |
| `VIDEO_FRAME_STRIDE` | `30` | Frame stride used when sampling uploaded video scenes for search. |
| `VIDEO_MAX_FRAMES` | empty | Optional maximum sampled frames per uploaded video scene. Falls back to `GIF_SAMPLE_FRAMES` when unset. |
| `GIF_SAMPLE_FRAMES` | `16` | Maximum sampled GIF frames used for vector generation. |
| `GIF_MAX_DECODE_FRAMES` | `512` | Maximum GIF frames decoded before deterministic truncation. |
| `GIF_PREVIEW_FRAMES` | `16` | Maximum frames written to generated animated GIF previews. |
| `GIF_DEFAULT_FRAME_DELAY_MS` | `100` | Delay used when a GIF frame delay is zero or missing. |
| `GIF_MOTION_WEIGHT` | `0.2` | Blend weight for motion deltas in animation-aware GIF vectors. |
| `SAMPLE_FACE_HOST_DIR` | `./sample-images` | Host folder used only when the optional seed profile is run. |
| `SAMPLE_FACE_DATA_DIR` | `/seed/local` | Container path where the optional seed job writes local sample images. |
| `SAMPLE_FACE_COUNT` | `150` | Number of example people images to download. |
| `SAMPLE_FACE_URL` | `https://thispersondoesnotexist.com/` | JPEG source used by the seed job. |
| `SAMPLE_FACE_DELAY_MS` | `1000` | Delay between download attempts, to avoid duplicate cached responses. |
| `SAMPLE_FACE_MAX_ATTEMPTS` | `750` | Maximum attempts allowed while collecting unique images. |
| `SAMPLE_FACE_CLEAR_GENERATED` | `true` | Remove prior generated `person-*.jpg` and legacy `dummy-*` files before seeding. |

## Source Examples

By default, source folders are configured in `config/media-sources.txt`:

```txt
# Local media folders indexed by default.
/media/pictures
/media/videos
/media/audio
```

The file uses a small `.gitignore`-style convention: blank lines and lines starting with `#` are ignored. Each remaining line is a local path or supported source URI. Local paths may use `~`, `$VAR`, or `${VAR}` expansion. The indexer scans configured image, video, audio, and PDF extensions under each listed folder.

Object-store sources use AWS S3-compatible listing and object fetches:

```txt
minio://media-bucket/photos
s3://archive-bucket/family/2024
```

Remote objects are downloaded into an ephemeral cache under `UPLOAD_DIR/source-cache` while they are decoded by the existing image, FFmpeg, and Poppler pipelines. The cache entry is removed after each object is indexed; generated thumbnails and clips still live under the configured data directories.

`IMAGE_SOURCES` still accepts multiple local source paths and overrides the file when set. Local folder support remains backward compatible with `SOURCE_IMAGE_DIR`.

```env
IMAGE_SOURCES='["local:///images", "/more-images"]'
```

Unsupported source types are surfaced in the index response as skipped sources instead of crashing the index run.

## Development Workflow

### Setup

Install the frontend dependencies:

```bash
bun install
```

Rust service builds and tests use Cargo with path dependencies from a sibling checkout:

```txt
../rust-packages
```

Install Playwright Chromium once before running UI end-to-end tests:

```bash
bunx playwright install chromium
```

### Daily Development

Start the full local app stack and the Vite frontend:

```bash
bun dev
```

This runs Docker Compose for the Rust app and Qdrant, then starts Vite. Use this lighter command when only the containers need to be refreshed:

```bash
bun run dev:containers
```

### Project Commands

| Command | Purpose |
| --- | --- |
| `bun run test` | Fast meaningful test path: Rust test suite. |
| `bun run test:e2e` | Playwright UI tests with mocked API responses. |
| `bun run lint` | TypeScript check, Rust format check, and Clippy with warnings denied. |
| `bun run format:check` | Frontend formatting check. |
| `bun run format:check:rust` | Rust formatting check. |
| `bun run format` | Write frontend formatting changes. |
| `bun run format:rust` | Write Rust formatting changes. |
| `bun run build` | Build the frontend into `frontend/dist`. |
| `bun run build:rust` | Build Rust service binaries. |
| `bun run check:hygiene` | Report dirty status, upstream state, and ignored/generated directory issues. |
| `bun run sample:check` | Validate the internet sample-corpus manifest. |
| `bun run sample:download` | Download showcase sample media into `sample-images/showcase`. |
| `bun run test:sample` | Run sample-corpus tests. |
| `bun run verify` | Full local confidence check. |

`frontend/dist` is generated Vite output that is intentionally checked in for the Rust service to serve. Update it only by running `bun run build`.

### Full Verification

Run this before handing off larger changes:

```bash
bun run verify
```

The verification command runs the hygiene report, frontend format check, TypeScript/Rust static checks, Rust tests, Playwright tests, and the frontend build. It requires the sibling `../rust-packages` checkout and Playwright Chromium.

### Release Notes

This repository does not currently have a release or publish command. The frontend package is private and the Rust crate has `publish = false`. Use the existing Docker build when a runtime image is needed:

```bash
docker build --build-context rust-packages=../rust-packages -t image-similarity-service .
```

### Troubleshooting

- If Rust commands cannot resolve `audio-analysis-*`, `image-analysis-*`, `vector-analysis-*`, or `video-analysis-*` crates, confirm `../rust-packages` exists.
- If `bun run test:e2e` fails before opening the UI, run `bunx playwright install chromium`.
- If video or audio indexing fails locally, confirm `ffmpeg` and `ffprobe` are installed and on `PATH`.
- If `git status --short` is noisy, run `bun run check:hygiene` and confirm local generated directories are ignored.

## Local Development

Run the backend locally:

```bash
QDRANT_URL=http://localhost:6333 \
MEDIA_SOURCES_FILE=config/media-sources.txt \
cargo run --manifest-path backend/Cargo.toml --bin image-similarity-service
```

Run the Rust test suite:

```bash
cargo test --manifest-path backend/Cargo.toml
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
cargo run --manifest-path backend/Cargo.toml --bin seed_dummy_data
```

Run the Compose seed job again:

```bash
docker compose --profile seed run --rm seed-data
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

The dev script starts the Docker Compose app stack first, then starts Vite. The Vite dev server proxies `/api` and `/thumbnails` to `http://127.0.0.1:8000`.

Start or refresh only the Docker containers used by the dev server:

```bash
bun run dev:containers
```

Build the frontend into the checked-in frontend dist directory:

```bash
bun run build
```

Format and type-check the frontend:

```bash
bun run format
bun run typecheck
```

Run the Playwright UI end-to-end tests:

```bash
bun run test:e2e
```

If Playwright browsers are not installed on the machine yet, install Chromium first:

```bash
bunx playwright install chromium
```

## Notes

- Re-running indexing upserts local images by deterministic ID based on absolute image path.
- If an image file changes at the same path, run indexing again to refresh its vector, pHash, metadata, and thumbnail.
- The Dockerfile uses Docker BuildKit's named `rust-packages` context for the sibling workspace required by the Cargo path dependencies.
- This version intentionally omits auth, users, async queues, and a separate frontend build pipeline.

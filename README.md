# Image Similarity Service

Dockerized local image similarity search using FastAPI, Qdrant, SentenceTransformers CLIP embeddings, and perceptual hashes.

The service indexes configured image sources, then lets you upload a query image through the web UI or HTTP API to find visually similar images and near duplicates.

## Features

- FastAPI backend with upload, indexing, search, and health endpoints.
- Pluggable indexing sources for local folders, MinIO buckets, video frames, and camera streams.
- Qdrant vector database for CLIP image embeddings.
- `sentence-transformers/clip-ViT-B-32` by default.
- `imagehash` pHash distance for duplicate and near-duplicate detection.
- Rust-backed PNG/JPEG/WebP image loading, thumbnail encoding, pHash DCT/hash assembly, and hash distance via the sibling Rust crates, with Python/Pillow fallback for full format compatibility and EXIF orientation handling.
- React UI built with Bun, TypeScript, React Query, Tailwind CSS, and oxfmt, served as static assets by FastAPI.
- Docker Compose setup with Qdrant, MinIO, and a deterministic dummy-data seed job.

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
   If you keep the default `./sample-images`, Docker Compose runs the `seed-data` job first and fills that directory with deterministic dummy images.

3. Start the service:

   ```bash
   docker compose up --build
   ```

4. Open the UI:

   ```txt
   http://localhost:8000
   ```

5. Click **Index configured sources**, then upload a query image and search.

The first indexing or search request can take longer because the CLIP model may need to download and load.
The Compose stack also starts MinIO at `http://localhost:9000` with the console at `http://localhost:9001`; the seed job uploads the same dummy images to `minio://image-similarity-dev/catalog`.

## API

### Health

```bash
curl http://localhost:8000/api/health
```

### Index Configured Sources

```bash
curl -X POST http://localhost:8000/api/index
```

### Search With Uploaded Image

```bash
curl -X POST "http://localhost:8000/api/search?limit=12" \
  -F "file=@/path/to/query.jpg"
```

The response includes:

- `vector_score`: Qdrant cosine similarity score from the CLIP embedding search.
- `hash_distance`: pHash Hamming distance from the query image.
- `near_duplicate`: `true` when `hash_distance <= DUPLICATE_HASH_DISTANCE`.
- `thumbnail_url`: URL for the generated thumbnail served by the backend.

## Configuration

Set these values in `.env`:

| Variable | Default | Purpose |
| --- | --- | --- |
| `HOST_SOURCE_IMAGE_DIR` | `./sample-images` | Host folder mounted into the app container. |
| `SOURCE_IMAGE_DIR` | `/images` | Container path scanned by the indexer. |
| `IMAGE_SOURCES` | empty | Optional source list. When empty, the indexer scans `SOURCE_IMAGE_DIR`. Use a JSON array, comma-separated list, semicolon-separated list, or newline-separated list. |
| `MINIO_ENDPOINT` | `minio:9000` | Default MinIO endpoint for `minio://...` sources inside Docker Compose. |
| `MINIO_ACCESS_KEY` | `minioadmin` | Default MinIO access key and Compose root user. |
| `MINIO_SECRET_KEY` | `minioadmin` | Default MinIO secret key and Compose root password. |
| `MINIO_SECURE` | `false` | Whether MinIO sources use TLS by default. |
| `MINIO_BUCKET` | `image-similarity-dev` | Bucket created by the dummy-data seed job. |
| `MINIO_PREFIX` | `catalog` | Object prefix populated by the dummy-data seed job. |
| `MINIO_CONSOLE_PORT` | `9001` | Host port for the MinIO browser console. |
| `VIDEO_FRAME_STRIDE` | `30` | Default video sampling interval; `30` indexes every 30th frame. |
| `VIDEO_MAX_FRAMES` | empty | Optional cap on indexed frames per video source. |
| `CAMERA_FRAME_STRIDE` | `30` | Default camera stream sampling interval. |
| `CAMERA_MAX_FRAMES` | `100` | Default cap on indexed frames per camera stream. |
| `QDRANT_URL` | `http://qdrant:6333` | Qdrant URL from inside the app container. |
| `QDRANT_COLLECTION` | `image_similarity` | Qdrant collection name. |
| `RUST_QDRANT_COLLECTION` | `image_similarity_rust` | Qdrant collection used by the Rust service in Compose. |
| `TEST_QDRANT_COLLECTION` | `image_similarity_test` | Qdrant collection used by the Compose test profile. |
| `CLIP_MODEL_NAME` | `sentence-transformers/clip-ViT-B-32` | SentenceTransformers image model. |
| `VECTOR_SIZE` | `512` | Qdrant vector size for the configured model. |
| `THUMBNAIL_DIR` | `/app/data/thumbnails` | Generated thumbnail storage. |
| `UPLOAD_DIR` | `/app/data/uploads` | Reserved local upload storage path. |
| `IMAGE_EXTENSIONS` | `.jpg,.jpeg,.png,.webp,.bmp,.tif,.tiff` | File extensions to index. |
| `DEFAULT_SEARCH_LIMIT` | `12` | Default result count. |
| `DUPLICATE_HASH_DISTANCE` | `8` | Max pHash distance for near-duplicate flag. |
| `MAX_UPLOAD_MB` | `20` | Maximum uploaded query image size. |
| `DUMMY_DATA_DIR` | `/seed/local` | Container path where the seed job writes local dummy images. |
| `DUMMY_IMAGE_COUNT` | `8` | Number of deterministic dummy images to generate. |
| `DUMMY_IMAGE_SIZE` | `640x480` | Dummy image dimensions as `WIDTHxHEIGHT`. |
| `DUMMY_IMAGE_FORMAT` | `JPEG` | Image format generated by the seed job. |
| `SEED_MINIO` | `true` | Whether the seed job uploads dummy images to MinIO. |
| `MINIO_WAIT_SECONDS` | `60` | How long the seed job waits for MinIO readiness. |

### Source Examples

`IMAGE_SOURCES` accepts multiple source URIs. Local folder support is backward compatible with `SOURCE_IMAGE_DIR`.

```env
IMAGE_SOURCES='["local:///images", "video:///videos/demo.mp4?every_n_frames=24&max_frames=250"]'
```

MinIO sources can use global MinIO settings:

```env
IMAGE_SOURCES=minio://catalog/products
MINIO_ENDPOINT=minio:9000
MINIO_ACCESS_KEY=minioadmin
MINIO_SECRET_KEY=minioadmin
MINIO_SECURE=false
```

The default Compose seed bucket can be indexed with:

```env
IMAGE_SOURCES=minio://image-similarity-dev/catalog
```

Or they can include source-specific settings:

```env
IMAGE_SOURCES='["minio://catalog/products?endpoint=minio:9000&access_key=minioadmin&secret_key=minioadmin&secure=false"]'
```

Camera sources use OpenCV capture targets. A local webcam can be configured as:

```env
IMAGE_SOURCES=camera://0?every_n_frames=10&max_frames=50
```

For RTSP or HTTP camera streams, URL-encode the stream URL in the `url` query parameter:

```env
IMAGE_SOURCES='["camera://?url=rtsp%3A%2F%2Fuser%3Apass%40camera.local%2Fstream&every_n_frames=30&max_frames=100"]'
```

## Local Development

Install dependencies:

```bash
python -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

Build the optional Rust extension after dependency installation:

```bash
python scripts/build_rust_extension.py
```

The extension uses the sibling `../rust-packages` crates for image I/O, image processing, pHash computation, and vector math. If it is not built, the service keeps the same behavior through the Python fallback paths.
Rebuild it while the service and tests are stopped, because the script replaces the local extension file in place.

### Native Rust service

The repository also contains a native Rust service binary:

```bash
cargo run --manifest-path rust/Cargo.toml --bin image-similarity-service
```

It serves the same `/`, `/static`, `/thumbnails`, `/api/health`, `/api/index`, and `/api/search` routes and uses the same environment variable names as the Python service. The Rust Docker image is runtime-Python-free:

```bash
docker build --build-context rust-packages=../rust-packages -f Dockerfile.rust -t image-similarity-service:rust .
```

Current Rust parity status:

- Local folder indexing, Rust image loading for JPEG/PNG/WebP/BMP/TIFF, pHashing, thumbnail generation, Qdrant REST upsert/search, and multipart search upload are implemented.
- The Rust embedder is a deterministic normalized image-vector implementation. It is a native placeholder for the Python SentenceTransformers CLIP embedder, not CLIP-equivalent inference.
- MinIO, video, and camera source URIs are parsed and reported as unavailable in the native Rust service until Rust-native storage and media capture backends are added.

Run the deterministic test suite:

```bash
PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 pytest -q
```

### Frontend development

Install frontend dependencies with Bun:

```bash
bun install
```

Run the React dev server:

```bash
bun run dev
```

The Vite dev server proxies `/api` and `/thumbnails` to `http://127.0.0.1:8000`, so run the FastAPI service separately while developing the UI.

Build the frontend into the backend static directory:

```bash
bun run build
```

Format and type-check the frontend:

```bash
bun run format
bun run typecheck
```

Run the same tests inside Docker Compose with Qdrant, MinIO, and seeded dummy data:

```bash
docker compose --profile test run --rm test
```

Run tests with a coverage report:

```bash
PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 pytest -q -p pytest_cov --cov=image_similarity --cov-report=term-missing
```

`PYTEST_DISABLE_PLUGIN_AUTOLOAD=1` keeps unrelated globally installed pytest plugins from affecting this project. The default tests use fakes for CLIP and Qdrant, so they do not download a model, start Docker, or require an external vector database.

Run only a marked test tier:

```bash
PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 pytest -q -m "unit or api"
PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 pytest -q -m integration
```

Run Qdrant:

```bash
docker compose up qdrant
```

Generate or refresh local dummy images without starting the full stack:

```bash
python scripts/seed_dummy_data.py
```

Run the Compose seed job again, including MinIO upload:

```bash
docker compose run --rm seed-data
```

Run the API locally:

```bash
SOURCE_IMAGE_DIR=/absolute/path/to/images \
QDRANT_URL=http://localhost:6333 \
uvicorn image_similarity.main:app --reload
```

## Benchmarks

Run the deterministic synthetic benchmark:

```bash
python benchmarks/benchmark_baseline.py --profile synthetic --output benchmarks/results/baseline.json
```

The synthetic benchmark generates deterministic images and measures image loading, pHash throughput, thumbnail generation, payload building, search response assembly, and synthetic indexing without CLIP or Qdrant.

Build and benchmark the Python fallback image against the Rust-backed image:

```bash
docker build -t image-similarity-service:python .
docker build --build-context rust-packages=../rust-packages -f Dockerfile.rust -t image-similarity-service:rust .

docker run --rm \
  -v "$PWD/benchmarks:/benchmarks" \
  image-similarity-service:python \
  python /benchmarks/benchmark_baseline.py --profile synthetic --output /benchmarks/results/python.json

docker run --rm \
  -v "$PWD/benchmarks:/benchmarks" \
  image-similarity-service:rust \
  python /benchmarks/benchmark_baseline.py --profile synthetic --output /benchmarks/results/rust.json
```

`Dockerfile.rust` uses Docker BuildKit's named `rust-packages` context for the sibling workspace required by the Cargo path dependencies. Docker Compose also exposes the Rust-backed service as `app-rust` on port `8001`, while the original `app` service remains on port `8000`.

Run the optional real-stack benchmark after starting Qdrant and installing the full project dependencies:

```bash
QDRANT_URL=http://localhost:6333 \
python benchmarks/benchmark_baseline.py --profile real --output benchmarks/results/real-baseline.json
```

The real profile initializes the configured SentenceTransformers CLIP model and Qdrant collection, so results depend on hardware, model cache state, and local service availability.

## Notes

- Re-running indexing upserts local images by deterministic ID based on absolute image path. MinIO objects and video frames use deterministic IDs based on their source URI. Camera stream frames use the configured capture target and sampled frame number.
- If an image file or object changes at the same path, run indexing again to refresh its vector, pHash, metadata, and thumbnail.
- This first version intentionally omits auth, users, async queues, and a separate frontend build pipeline.

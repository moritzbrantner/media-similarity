# Image Similarity Service

Dockerized local image similarity search using FastAPI, Qdrant, SentenceTransformers CLIP embeddings, and perceptual hashes.

The service indexes a mounted source image folder, then lets you upload a query image through the web UI or HTTP API to find visually similar images and near duplicates.

## Features

- FastAPI backend with upload, indexing, search, and health endpoints.
- Qdrant vector database for CLIP image embeddings.
- `sentence-transformers/clip-ViT-B-32` by default.
- `imagehash` pHash distance for duplicate and near-duplicate detection.
- Rust-backed PNG/JPEG/WebP image loading, thumbnail encoding, and hash distance via the sibling Rust crates, with Pillow fallback for full format compatibility and EXIF orientation handling.
- Plain static HTML/CSS/JS UI served by FastAPI.
- Docker Compose setup with a single source-folder mount to edit.

## Quick Start

1. Copy the environment file:

   ```bash
   cp .env.example .env
   ```

2. Edit `.env` and set `HOST_SOURCE_IMAGE_DIR` to your local image folder:

   ```env
   HOST_SOURCE_IMAGE_DIR=/absolute/path/to/your/images
   SOURCE_IMAGE_DIR=/images
   ```

   `HOST_SOURCE_IMAGE_DIR` is used by Docker Compose on the host. `SOURCE_IMAGE_DIR` is the path inside the app container and can usually stay `/images`.

3. Start the service:

   ```bash
   docker compose up --build
   ```

4. Open the UI:

   ```txt
   http://localhost:8000
   ```

5. Click **Index source folder**, then upload a query image and search.

The first indexing or search request can take longer because the CLIP model may need to download and load.

## API

### Health

```bash
curl http://localhost:8000/api/health
```

### Index Mounted Source Folder

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
| `QDRANT_URL` | `http://qdrant:6333` | Qdrant URL from inside the app container. |
| `QDRANT_COLLECTION` | `image_similarity` | Qdrant collection name. |
| `CLIP_MODEL_NAME` | `sentence-transformers/clip-ViT-B-32` | SentenceTransformers image model. |
| `VECTOR_SIZE` | `512` | Qdrant vector size for the configured model. |
| `THUMBNAIL_DIR` | `/app/data/thumbnails` | Generated thumbnail storage. |
| `UPLOAD_DIR` | `/app/data/uploads` | Reserved local upload storage path. |
| `IMAGE_EXTENSIONS` | `.jpg,.jpeg,.png,.webp,.bmp,.tif,.tiff` | File extensions to index. |
| `DEFAULT_SEARCH_LIMIT` | `12` | Default result count. |
| `DUPLICATE_HASH_DISTANCE` | `8` | Max pHash distance for near-duplicate flag. |
| `MAX_UPLOAD_MB` | `20` | Maximum uploaded query image size. |

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

The extension uses the sibling `../rust-packages` crates for image I/O, image processing, and vector math. If it is not built, the service keeps the same behavior through the Python fallback paths.
Rebuild it while the service and tests are stopped, because the script replaces the local extension file in place.

Run the deterministic test suite:

```bash
PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 pytest -q
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

Run the optional real-stack benchmark after starting Qdrant and installing the full project dependencies:

```bash
QDRANT_URL=http://localhost:6333 \
python benchmarks/benchmark_baseline.py --profile real --output benchmarks/results/real-baseline.json
```

The real profile initializes the configured SentenceTransformers CLIP model and Qdrant collection, so results depend on hardware, model cache state, and local service availability.

## Notes

- Re-running indexing upserts images by deterministic ID based on absolute image path.
- If an image file changes at the same path, run indexing again to refresh its vector, pHash, metadata, and thumbnail.
- This first version intentionally omits auth, users, async queues, and a separate frontend build pipeline.

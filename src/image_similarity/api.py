from __future__ import annotations

from functools import lru_cache
from io import BytesIO

from fastapi import APIRouter, Depends, File, HTTPException, UploadFile
from PIL import Image, ImageOps, UnidentifiedImageError

from image_similarity.config import Settings, get_settings
from image_similarity.embeddings import ImageEmbedder
from image_similarity.indexer import ImageIndexer
from image_similarity.models import HealthResponse, IndexResponse, SearchResponse
from image_similarity.qdrant_store import QdrantImageStore
from image_similarity.search import ImageSearchService
from image_similarity.sources import build_image_sources

router = APIRouter(prefix="/api")


@lru_cache
def _cached_embedder(model_name: str) -> ImageEmbedder:
    return ImageEmbedder(model_name)


@lru_cache
def _cached_store(url: str, collection: str, vector_size: int) -> QdrantImageStore:
    return QdrantImageStore(url, collection, vector_size)


def get_embedder(settings: Settings = Depends(get_settings)) -> ImageEmbedder:
    return _cached_embedder(settings.clip_model_name)


def get_store(settings: Settings = Depends(get_settings)) -> QdrantImageStore:
    return _cached_store(settings.qdrant_url, settings.qdrant_collection, settings.vector_size)


def get_indexer(
    settings: Settings = Depends(get_settings),
    store: QdrantImageStore = Depends(get_store),
    embedder: ImageEmbedder = Depends(get_embedder),
) -> ImageIndexer:
    return ImageIndexer(settings, store, embedder)


def get_search_service(
    settings: Settings = Depends(get_settings),
    store: QdrantImageStore = Depends(get_store),
    embedder: ImageEmbedder = Depends(get_embedder),
) -> ImageSearchService:
    return ImageSearchService(settings, store, embedder)


@router.get("/health", response_model=HealthResponse)
def health(settings: Settings = Depends(get_settings)) -> HealthResponse:
    sources = build_image_sources(settings)
    return HealthResponse(
        status="ok",
        collection=settings.qdrant_collection,
        source_dir=str(settings.source_image_dir),
        sources=[source.uri for source in sources],
    )


@router.post("/index", response_model=IndexResponse)
def index_images(indexer: ImageIndexer = Depends(get_indexer)) -> IndexResponse:
    if hasattr(indexer, "index_sources"):
        return indexer.index_sources()
    return indexer.index_source_dir()


@router.post("/search", response_model=SearchResponse)
async def search_upload(
    file: UploadFile = File(...),
    limit: int | None = None,
    settings: Settings = Depends(get_settings),
    search_service: ImageSearchService = Depends(get_search_service),
) -> SearchResponse:
    content_type = file.content_type or ""
    if not content_type.startswith("image/"):
        raise HTTPException(status_code=400, detail="Upload must be an image file")

    raw = await file.read()
    max_bytes = settings.max_upload_mb * 1024 * 1024
    if len(raw) > max_bytes:
        raise HTTPException(status_code=413, detail=f"Upload is larger than {settings.max_upload_mb} MB")

    try:
        with Image.open(BytesIO(raw)) as image:
            query_image = ImageOps.exif_transpose(image).convert("RGB")
    except UnidentifiedImageError as exc:
        raise HTTPException(status_code=400, detail="Could not decode image") from exc

    return search_service.search_image(query_image, limit=limit)

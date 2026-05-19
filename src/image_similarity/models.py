from pydantic import BaseModel, Field


class ImagePayload(BaseModel):
    id: str
    path: str
    relative_path: str
    filename: str
    width: int
    height: int
    size_bytes: int
    modified_at: float
    phash: str
    thumbnail_url: str | None = None
    source_type: str = "local"
    source_uri: str | None = None


class SearchResult(BaseModel):
    image: ImagePayload
    vector_score: float = Field(description="Qdrant similarity score for the CLIP vector search.")
    hash_distance: int | None = Field(default=None, description="pHash Hamming distance from the query.")
    near_duplicate: bool = False


class SearchResponse(BaseModel):
    query_phash: str
    count: int
    results: list[SearchResult]


class IndexResponse(BaseModel):
    indexed: int
    skipped: int
    failed: int
    collection: str
    source_dir: str
    sources: list[str] = Field(default_factory=list)
    errors: list[str] = Field(default_factory=list)


class HealthResponse(BaseModel):
    status: str
    collection: str
    source_dir: str
    sources: list[str] = Field(default_factory=list)

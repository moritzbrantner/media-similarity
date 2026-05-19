import pytest

from image_similarity.models import HealthResponse, ImagePayload, IndexResponse, SearchResponse, SearchResult

pytestmark = pytest.mark.unit


def sample_payload() -> ImagePayload:
    return ImagePayload(
        id="image-id",
        path="/images/cat.jpg",
        relative_path="cat.jpg",
        filename="cat.jpg",
        width=64,
        height=48,
        size_bytes=1234,
        modified_at=1.5,
        phash="0000000000000000",
    )


def test_image_payload_defaults_and_serialization() -> None:
    payload = sample_payload()

    assert payload.thumbnail_url is None
    assert payload.source_type == "local"
    assert payload.source_uri is None
    assert payload.model_dump()["relative_path"] == "cat.jpg"


def test_search_result_and_response_defaults() -> None:
    result = SearchResult(image=sample_payload(), vector_score=0.75)
    response = SearchResponse(query_phash="ffffffffffffffff", count=1, results=[result])

    assert result.hash_distance is None
    assert result.near_duplicate is False
    assert response.model_dump()["results"][0]["vector_score"] == 0.75


def test_index_response_errors_default_to_empty_list() -> None:
    response = IndexResponse(indexed=1, skipped=0, failed=0, collection="images", source_dir="/images")

    assert response.errors == []
    assert response.sources == []


def test_health_response_shape() -> None:
    response = HealthResponse(status="ok", collection="images", source_dir="/images")

    assert response.model_dump() == {
        "status": "ok",
        "collection": "images",
        "source_dir": "/images",
        "sources": [],
    }

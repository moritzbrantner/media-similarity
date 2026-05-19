from __future__ import annotations

import importlib
import sys
from pathlib import Path

import pytest
from fastapi.routing import Mount
from fastapi.testclient import TestClient

pytestmark = [pytest.mark.unit, pytest.mark.api]


def test_main_app_exposes_index_and_static_mounts(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("THUMBNAIL_DIR", str(tmp_path / "thumbnails"))
    monkeypatch.setenv("UPLOAD_DIR", str(tmp_path / "uploads"))
    sys.modules.pop("sentence_transformers", None)
    sys.modules.pop("image_similarity.main", None)

    main = importlib.import_module("image_similarity.main")

    response = TestClient(main.app).get("/")
    mounted = {route.path for route in main.app.routes if isinstance(route, Mount)}
    assert response.status_code == 200
    assert "/static" in mounted
    assert "/thumbnails" in mounted
    assert "sentence_transformers" not in sys.modules

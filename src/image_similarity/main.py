from pathlib import Path

from fastapi import FastAPI
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles

from image_similarity.api import router as api_router
from image_similarity.config import get_settings

app = FastAPI(title="Image Similarity Service", version="0.1.0")
app.include_router(api_router)

settings = get_settings()
settings.thumbnail_dir.mkdir(parents=True, exist_ok=True)
settings.upload_dir.mkdir(parents=True, exist_ok=True)

static_dir = Path(__file__).parent / "static"
app.mount("/static", StaticFiles(directory=static_dir), name="static")
app.mount("/thumbnails", StaticFiles(directory=settings.thumbnail_dir), name="thumbnails")


@app.get("/", include_in_schema=False)
def index() -> FileResponse:
    return FileResponse(static_dir / "index.html")


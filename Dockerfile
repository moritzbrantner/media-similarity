FROM python:3.11-slim

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends libgomp1 curl \
    && rm -rf /var/lib/apt/lists/*

COPY pyproject.toml README.md ./
COPY src ./src

RUN pip install --upgrade pip \
    && pip install .

COPY scripts ./scripts

RUN mkdir -p /app/data/thumbnails /app/data/uploads /images

EXPOSE 8000

CMD ["uvicorn", "image_similarity.main:app", "--host", "0.0.0.0", "--port", "8000"]

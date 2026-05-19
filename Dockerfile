FROM oven/bun:1.3.14 AS frontend-builder

WORKDIR /workspace

COPY package.json bun.lock tsconfig.json vite.config.ts .oxfmtrc.json ./
COPY src/frontend ./src/frontend

RUN bun install --frozen-lockfile \
    && bun run build

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
COPY --from=frontend-builder /workspace/src/image_similarity/static ./src/image_similarity/static

RUN pip install --upgrade pip \
    && pip install .

COPY scripts ./scripts

RUN mkdir -p /app/data/thumbnails /app/data/uploads /images

EXPOSE 8000

CMD ["uvicorn", "image_similarity.main:app", "--host", "0.0.0.0", "--port", "8000"]

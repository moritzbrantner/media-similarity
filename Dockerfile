FROM oven/bun:1.3.14 AS frontend-builder

WORKDIR /workspace

COPY package.json bun.lock tsconfig.json vite.config.ts .oxfmtrc.json ./
COPY frontend ./frontend

RUN bun install --frozen-lockfile \
    && bun run build

FROM rust:1-bookworm AS rust-builder

WORKDIR /workspace/image-similarity-service

COPY --from=rust-packages . /workspace/rust-packages
COPY backend ./backend

RUN cargo build --manifest-path backend/Cargo.toml --bins --release

FROM debian:bookworm-slim

ENV RUST_LOG=info
ENV FRONTEND_DIST_DIR=/app/frontend/dist

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        ffmpeg \
        poppler-utils \
        tesseract-ocr \
        tesseract-ocr-eng \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /workspace/image-similarity-service/backend/target/release/image-similarity-service /usr/local/bin/image-similarity-service
COPY --from=rust-builder /workspace/image-similarity-service/backend/target/release/seed_dummy_data /usr/local/bin/seed_dummy_data
COPY --from=frontend-builder /workspace/frontend/dist ./frontend/dist
COPY config ./config

RUN mkdir -p /app/data/thumbnails /app/data/uploads /images /media/pictures /media/videos /media/audio

EXPOSE 8000

CMD ["image-similarity-service"]

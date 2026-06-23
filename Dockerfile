FROM oven/bun:1.3.14 AS frontend-builder

ENV PUPPETEER_SKIP_DOWNLOAD=true

WORKDIR /workspace

COPY package.json bun.lock tsconfig.json vite.config.ts .oxfmtrc.json ./
COPY frontend ./frontend

RUN bun install --frozen-lockfile \
    && bun run build

FROM rust:1-bookworm AS rust-builder

WORKDIR /workspace/image-similarity-service

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-packages . /workspace/rust-packages
COPY backend ./backend

RUN cargo build --manifest-path backend/Cargo.toml --bins --release

FROM debian:bookworm-slim AS api-runtime

ENV RUST_LOG=info

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
COPY config ./config

RUN mkdir -p /app/data/thumbnails /app/data/uploads /images /media/pictures /media/videos /media/audio

EXPOSE 8000

CMD ["image-similarity-service"]

FROM nginx:1.27-alpine AS web-runtime

ENV MAX_UPLOAD_MB=20

COPY --from=frontend-builder /workspace/frontend/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/templates/default.conf.template

EXPOSE 80

FROM api-runtime AS app

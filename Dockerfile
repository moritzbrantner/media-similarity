FROM oven/bun:1.3.14 AS frontend-builder

WORKDIR /workspace

COPY package.json bun.lock tsconfig.json vite.config.ts .oxfmtrc.json ./
COPY src/frontend ./src/frontend

RUN bun install --frozen-lockfile \
    && bun run build

FROM rust:1-bookworm AS rust-builder

WORKDIR /workspace/image-similarity-service

COPY --from=rust-packages . /workspace/rust-packages
COPY rust ./rust

RUN cargo build --manifest-path rust/Cargo.toml --bins --release

FROM debian:bookworm-slim

ENV RUST_LOG=info

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        ffmpeg \
        tesseract-ocr \
        tesseract-ocr-eng \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /workspace/image-similarity-service/rust/target/release/image-similarity-service /usr/local/bin/image-similarity-service
COPY --from=rust-builder /workspace/image-similarity-service/rust/target/release/seed_dummy_data /usr/local/bin/seed_dummy_data
COPY --from=frontend-builder /workspace/src/image_similarity/static ./src/image_similarity/static
COPY config ./config

RUN mkdir -p /app/data/thumbnails /app/data/uploads /images /media/pictures /media/videos /media/audio

EXPOSE 8000

CMD ["image-similarity-service"]

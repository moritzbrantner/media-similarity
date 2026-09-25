FROM oven/bun:1.3.14@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4 AS frontend-builder

ENV PUPPETEER_SKIP_DOWNLOAD=true

WORKDIR /workspace

COPY package.json bun.lock tsconfig.json vite.config.ts .oxfmtrc.json ./
COPY frontend ./frontend

RUN bun install --frozen-lockfile \
    && bun run build

FROM rust:1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS rust-builder

WORKDIR /workspace/image-similarity-service

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake \
    && rm -rf /var/lib/apt/lists/*

COPY rust-toolchain.toml ./rust-toolchain.toml
RUN toolchain="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml)" \
    && test -n "$toolchain" \
    && rustup toolchain install "$toolchain" --profile minimal \
    && test "$(rustc --version | awk '{print $2}')" = "$toolchain"

COPY backend ./backend

RUN cargo build --manifest-path backend/Cargo.toml --bins --release

FROM debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251 AS api-runtime

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
COPY --from=rust-builder /workspace/image-similarity-service/backend/target/release/quality_corpus /usr/local/bin/quality_corpus
COPY --from=rust-builder /workspace/image-similarity-service/backend/target/release/seed_dummy_data /usr/local/bin/seed_dummy_data
COPY config ./config

RUN mkdir -p /app/data/thumbnails /app/data/uploads /images /media/pictures /media/videos /media/audio

EXPOSE 8000

CMD ["image-similarity-service"]

FROM nginx:1.27-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10 AS web-runtime

ENV MAX_UPLOAD_MB=20

COPY --from=frontend-builder /workspace/frontend/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/templates/default.conf.template

EXPOSE 80

FROM api-runtime AS app

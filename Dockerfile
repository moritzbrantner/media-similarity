FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8 AS frontend-builder

ENV PUPPETEER_SKIP_DOWNLOAD=true
ENV PATH="/root/.bun/bin:${PATH}"

WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && rm -rf /var/lib/apt/lists/*

COPY package.json bun.lock tsconfig.json vite.config.ts .oxfmtrc.json ./
RUN bun_version="$(sed -n 's/.*"packageManager": "bun@\([^"]*\)".*/\1/p' package.json)" \
    && test -n "$bun_version" \
    && curl -fsSL https://bun.sh/install | bash -s "bun-v${bun_version}" \
    && test "$(bun --version)" = "$bun_version"

COPY frontend ./frontend

RUN bun install --frozen-lockfile \
    && bun run build

FROM rust:1-bookworm@sha256:edaa2a8cc254d5936207af267114814c61cceea40b46eaa4e976068af55e20e7 AS rust-builder

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

FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8 AS api-runtime

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

FROM nginx:1.31-alpine@sha256:060849a174666c53439dc7cecbf8f67e225c7cbfb1a44ca97915d9a172a81e6d AS web-runtime

ENV MAX_UPLOAD_MB=20

COPY --from=frontend-builder /workspace/frontend/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/templates/default.conf.template

EXPOSE 80

FROM api-runtime AS app

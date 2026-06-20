# AGENTS.md

## Project Purpose

Native Rust media similarity service with a React/Vite frontend. The backend indexes local image, GIF, video, and audio sources, writes thumbnails/uploads under local data directories, stores vectors in Qdrant, and serves the checked-in static frontend bundle.

## Repository Map

- `backend/src/`: Rust Axum service, indexing, media decoding, pHash/vector search, Qdrant integration, and thumbnail/upload handling.
- `backend/src/app.rs`: Backend orchestration: settings, shared state, routes, static frontend serving, and Axum startup.
- `backend/src/workers/`: Rust worker modules for indexing, search, sources, media decoding/analysis, OCR, face/voice handling, thumbnails, and embeddings. These are modules inside the single backend process, not separate containers.
- `backend/tests/`: Rust integration tests.
- `frontend/`: React, TypeScript, Tailwind, and React Query frontend source.
- `frontend/dist/`: Vite build output served by the Rust backend. This is generated and checked in; update it with `bun run build`, not by manual edits.
- `tests/e2e/`: Playwright UI tests. API and thumbnail routes are mocked in the tests.
- `.github/workflows/tests.yml`: CI format, frontend build, Rust clippy, and Rust test workflow.
- `docker-compose.yml`: Local app, Qdrant, seed-data, and Rust test containers.

## Required Local Shape

- Use Bun for frontend commands. The lockfile is `bun.lock`.
- Rust path dependencies expect the sibling repository at `../rust-packages`.
- Docker Compose is needed for `bun dev` and the full app stack.
- `ffmpeg` and `ffprobe` are required for video/audio runtime behavior.
- Playwright needs Chromium installed once with `bunx playwright install chromium`.

## Standard Commands

- Install frontend dependencies: `bun install`
- Start the development stack and Vite UI: `bun dev`
- Start only Docker services used by the dev server: `bun run dev:containers`
- Fast meaningful test suite: `bun run test`
- UI end-to-end tests: `bun run test:e2e`
- Static checks: `bun run lint`
- Frontend format check: `bun run format:check`
- Rust format check: `bun run format:check:rust`
- Write frontend formatting changes: `bun run format`
- Write Rust formatting changes: `bun run format:rust`
- Frontend production build into checked-in static assets: `bun run build`
- Rust binary build: `bun run build:rust`
- Full local verification: `bun run verify`
- Repo hygiene report: `bun run check:hygiene`

There is no safe release or publish script in this repo. The root package is private and the Rust crate has `publish = false`.

## Verification Notes

`bun run verify` runs the hygiene report, frontend format check, TypeScript/Rust static checks, Rust tests, Playwright tests, and frontend build. It requires `../rust-packages` and Playwright Chromium. It may be slower than the fast Rust test command.

Always run `bun run lint` before finishing a job. If linting cannot be run, report the blocker in the final response.

CI intentionally keeps frontend and Rust jobs separate. Do not change existing `build` or `format:check` behavior without checking `.github/workflows/tests.yml`.

## Files Agents Should Not Edit Manually

- `bun.lock` and `backend/Cargo.lock`: update only through the package manager.
- `frontend/dist/**`: generated Vite output; update with `bun run build`.
- `node_modules/`, `backend/target/`, `data/`, `backend/data/`, `sample-images/`, `uploads/`, `thumbnails/`, `playwright-report/`, `test-results/`, and `benchmarks/results/`: local/generated directories that should stay ignored.
- `.env` and `.env.local`: local configuration. Keep `.env.example` checked in.

## Search And Orientation

- Prefer `rg --files` to list project files and `rg '<pattern>'` for text search.
- Use `git status --short` before edits and before final reporting.
- Use `bun run check:hygiene` when checkpoint noise or local generated files look suspicious.

## Expensive Or Stateful Commands

- `bun dev` starts Docker Compose and leaves containers running.
- `docker compose up --build -d` can rebuild the Rust app image.
- `cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings` and `cargo test --manifest-path backend/Cargo.toml` depend on `../rust-packages`.
- `bun run build` rewrites the checked-in static frontend bundle under `frontend/dist`.
- Seed commands can download sample face images into `sample-images/`.

Python support and PyO3 extension packaging have been removed. Do not add Python package entrypoints or Python benchmark/test harnesses unless the project explicitly reintroduces them.

## Docker And Codex/T3 Cleanup

This project uses Docker services for local development or tests: `qdrant`.

Standard commands:

- `services:up`: start persistent local development services.
- `services:down`: stop persistent local development services without deleting volumes.
- `services:clean`: remove compose services, orphans, and disposable volumes.
- `services:ps`: show compose service state.
- `test:with-services`: run Docker-dependent tests through trap-based cleanup.
- `check:hygiene`: print git status, upstream/ahead-behind, and Docker state.

Codex/T3 rules:

- Run `git status --short` before editing and before the final response.
- Run `bun run lint` before the final response, and report any lint failures or blockers.
- Preserve unrelated dirty files; do not mix them into commits.
- Prefer `test:with-services` for tests that need Docker.
- Do not run raw `docker compose up` for tests unless you also add trap-based cleanup.
- Before finalizing Docker-sensitive work, run `services:ps` or `docker compose ps` and report remaining containers.
- Use `services:down` after dev sessions. Use `services:clean` only when deleting disposable test data is acceptable.
- Format touched files only unless the task explicitly asks for a repo-wide format.

## Agent skills

This repo is configured for the Matt Pocock workflow skills and the agent-loop control plane.

- Issue tracker: `docs/agents/issue-tracker.md`
- Triage labels: `docs/agents/triage-labels.md`
- Domain context: `docs/agents/domain.md`
- Planning workflow: `docs/agents/planning-workflow.md`

### Planning workflow

Substantial new work should be planned into GitHub PRD issues instead of implemented directly. See `docs/agents/planning-workflow.md`.

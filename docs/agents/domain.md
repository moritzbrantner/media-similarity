# Domain Context

This is a single-context repository.

Agents should use the root `AGENTS.md` as the primary workflow context and the root `CONTEXT.md` as the project glossary. This is a single-context repository; there is no `CONTEXT-MAP.md`.

Important domain summary:

- Backend: native Rust Axum media similarity service.
- Frontend: React, TypeScript, Vite, Tailwind, and React Query.
- Storage/search: local media directories, generated thumbnails/uploads, and Qdrant vectors.
- Checked-in static frontend output lives in `frontend/dist/` and must be updated with `bun run build`, not manual edits.

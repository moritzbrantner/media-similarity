# Domain Context

This is a single-context repository.

Agents should use the root `AGENTS.md` as the primary domain and workflow context. There is currently no root `CONTEXT.md`, `CONTEXT-MAP.md`, or `docs/adr/` directory to read.

Important domain summary:

- Backend: native Rust Axum media similarity service.
- Frontend: React, TypeScript, Vite, Tailwind, and React Query.
- Storage/search: local media directories, generated thumbnails/uploads, and Qdrant vectors.
- Checked-in static frontend output lives in `frontend/dist/` and must be updated with `bun run build`, not manual edits.

# Dockerfile conventions

## DOCKERFILE-001 — Keep runtime images minimal and reproducible

- Prefer multi-stage builds when build and runtime requirements differ.
- Put only runtime requirements and intentional assets in the final image.

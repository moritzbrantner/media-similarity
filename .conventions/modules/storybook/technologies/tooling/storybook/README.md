# Storybook conventions

## STORYBOOK-001 — Stories are executable UI contracts

- Keep meaningful reusable UI states in deterministic, colocated stories with explicit fixtures or mocks.
- Do not depend on live backends or enumerate incidental prop combinations.

## STORYBOOK-002 — Build and audit Storybook in automation

- Treat the static build and accessibility audit of every included story as deterministic automation gates.

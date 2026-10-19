# CI verification ownership

`Verify` owns application verification and calls the independent validation workflows. The active default-branch ruleset requires `merge-ready` with strict base-branch freshness and no bypass actors; individual job names remain unchanged.

| Coverage | Canonical PR owner |
| --- | --- |
| Frontend formatting, TypeScript, production bundle freshness, Storybook, browser workflows and Unlighthouse budgets | `frontend-verify` |
| Backend formatting, all-target/all-feature Clippy, locked binary build and full Rust tests | `rust-verify` |
| Standalone text-shim formatting, Clippy, build and library tests | `rust-verify` |
| Native similarity-core formatting/build/test/Clippy, wasm32 checking and Pages build | `pages-validation / build` |
| Shared policy lint and repository conformance, including Rust-specific policy diagnostics | `foundation-validation / foundation` |
| Real Qdrant integration | `qdrant-smoke` |
| Deterministic instruction regression comparison | `performance-validation / Iai-Callgrind smoke` |
| Cold setup and container-build reproducibility | `environment-validation / cold-setup` |
| Materialized quality-corpus contracts | `asset-validation / materialize` |

The foundation workflow selects the root `policy` tier for lint instead of the complete `fast` capability tier. It still runs strict convention checks and runs repository conformance on every invocation, so source-only changes retain Rust policy diagnostics. The local `fast` and `full` tiers remain available with their existing capabilities.

The frontend production build already includes the same TypeScript and Vite checks as the root build command. The backend full test invocation includes its library tests; it must not be repeated through the root package's `test:unit` alias and again as a separate backend library run. Standalone shims retain their own tests and committed Cargo-generated lockfiles. The Pages job retains the native core build formerly performed by foundation.

Cold-environment compilation, model-independent corpus materialization, native/WASM target validation, real Qdrant integration and measured benchmarks remain distinct checks. They intentionally exercise different environments or behavior; this change does not convert them to skipped successes or alter their thresholds, fixtures or timeout limits.

## Measurement

Issue #87 recorded an approximately 40.1 job-runner-minute baseline before duplicate Tests/Verify workflows and feature-branch push triggers were removed in #93. Renovate PR #97, head `3e2ebfc488d7c7cf90bd2198e5dcb81b8ccbb7c5`, [Verify run 39326180575](https://github.com/moritzbrantner/media-similarity/actions/runs/39326180575), subsequently passed in **31.23 summed job-runner minutes**. This is hosted scheduling/cost evidence, not controlled application latency. Durations are each completed non-skipped job's `completed_at - started_at`, summed across the workflow.

That run still spent 6.57 job-runner minutes in foundation. Its fast gate repeated the backend suite via the root package, then repeated backend library tests, builds and formatting already owned by Verify/Pages. The policy-tier change removes those duplicate application runs while preserving the distinct checks above. New source-PR and Renovate measurements remain pending until CI executes the changed topology; no additional savings are claimed yet.

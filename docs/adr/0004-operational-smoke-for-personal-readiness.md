# Use operational smoke for personal-use readiness

We use an operational smoke gate, rather than ranking-quality thresholds, as the first personal-use readiness standard. The gate proves that the Linux Docker service stack can boot, report readiness, index the sample corpus, search every supported local media kind, serve generated artifacts, and shut down cleanly, while model-quality acceptance remains a separate quality-corpus concern.

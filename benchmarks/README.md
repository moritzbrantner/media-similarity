# Benchmarks

The baseline benchmark is a standalone script so it can run without pytest plugins.

Run the deterministic synthetic profile:

```bash
python benchmarks/benchmark_baseline.py --profile synthetic --output benchmarks/results/baseline.json
```

The synthetic profile does not load CLIP, start Docker, or connect to Qdrant. It generates deterministic images and measures image loading, pHashing, thumbnail generation, payload building, search response assembly, and synthetic indexing.

Run the optional real-stack profile with Qdrant available:

```bash
QDRANT_URL=http://localhost:6333 \
python benchmarks/benchmark_baseline.py --profile real --output benchmarks/results/real-baseline.json
```

The real profile initializes the configured CLIP model and Qdrant collection. Its numbers depend on hardware, model cache state, and Qdrant availability.

`benchmarks/results/` is ignored by git. To keep a dated baseline, write a named output such as:

```bash
python benchmarks/benchmark_baseline.py --profile synthetic --output benchmarks/results/baseline-2026-05-19.json
```

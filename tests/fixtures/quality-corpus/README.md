# Quality Corpus

This directory contains the manifest for the public, reproducible corpus used to
evaluate visual similarity and face recognition quality.

The media files are intentionally not checked in. Generate them under the
ignored `sample-images/quality` directory:

```bash
bun run quality:download
```

Generated evaluation reports are written under `benchmarks/results/`.

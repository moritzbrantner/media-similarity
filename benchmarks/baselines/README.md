# Benchmark Baselines

Rust benchmark targets live under `backend/benches/` and run through Cargo:

```bash
bun run bench:rust
```

Use the quick profile as a smoke check while developing:

```bash
bun run bench:rust:quick
```

Criterion writes generated reports under `backend/target/criterion`. Keep checked-in
baseline notes small and human-readable; generated result artifacts belong under
`benchmarks/results/`, which remains ignored.

Initial performance expectations:

- Static image decode plus pHash plus thumbnail generation should remain under
  roughly 10 ms for 256 px synthetic images on a typical developer machine.
- Filter construction and response serialization should grow linearly with
  result count.
- Duplicate grouping and inverse-index aggregation should not show unexpected
  quadratic behavior at the 1,000 to 10,000 record scale.
- Audio, video, and PDF benchmark groups may report that they were skipped when
  `ffmpeg`, `ffprobe`, or Poppler tools are not installed.

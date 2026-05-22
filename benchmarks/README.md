# Benchmarks

Benchmark code should be Rust-first. Put Cargo benchmarks under `backend/benches/` when they are added so they run through the Rust toolchain.

Use this directory for shared benchmark inputs, notes, and generated result artifacts that are not tied to a single Cargo benchmark target. Generated outputs belong in `benchmarks/results/`, which remains ignored.

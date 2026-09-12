# Quality Corpus

This directory contains the manifests and relevance truth for the public,
reproducible corpus used to evaluate media similarity and recognition quality.
The media bytes are intentionally not checked in; generated material lives under
the ignored `sample-images/quality` directory.

## Baseline all-media corpus

Validate and download the existing all-media corpus with:

```bash
bun run quality:check
bun run quality:download
```

The model-backed evaluator remains available through:

```bash
bun run quality:evaluate
```

## Asset-tooling image quality extension

The `asset-tooling/` directory adds a harder image-quality layer without moving
benchmark semantics into the asset build system:

- `contract.json` pins the exact accepted `moritzbrantner/asset-tooling` revision
  and the public subpaths used by the corpus generator.
- `catalog.json` defines externally acquired source images with immutable SHA-256
  and byte-length pins plus license evidence.
- `recipes.json` defines deterministic image perturbation recipes and generated
  query identities.
- `relationships.json` stays owned by `media-similarity`; it defines graded
  positives and hard negatives used to measure retrieval quality.

A local `asset-tooling` checkout must be at the exact revision in
`contract.json`. By default it is expected at `../asset-tooling`; override this
with `ASSET_TOOLING_ROOT=/absolute/path` when necessary. Generation fails closed
when the checked-out revision differs or tracked asset-tooling source files are
modified.

The fast contract check is part of normal tests and requires no network, model,
or sibling checkout:

```bash
bun run quality:asset-tooling:check
```

Materialize the verified baseline corpus plus deterministic image perturbations:

```bash
bun run quality:asset-tooling:prepare
```

The generator uses asset-tooling's catalog acquisition, content-addressed object
store, standard-image codec operations, and deterministic perturbation recipes.
It writes `asset-tooling-provenance.json` beside the generated media so every
query records its source content identity, recipe identity, operation build
identity, implementation identity, and output content identity.

Run the existing production quality evaluator against the extended corpus and
then compute graded retrieval metrics with:

```bash
bun run quality:asset-tooling:evaluate
```

The normal Rust evaluator is reused unchanged for indexing/search semantics. A
temporary compatibility manifest is generated under the ignored quality output
and supplied through `QUALITY_REPO_ROOT`; relevance truth is not duplicated into
asset-tooling.

The metric report contains:

- Recall@1 and Recall@5;
- mean reciprocal rank (MRR);
- nDCG@5 from graded relevance labels;
- hard-negative false-positive rate within the top five results;
- per-perturbation-family breakdowns.

Generated reports are written under `benchmarks/results/`, including the raw
extended evaluator output and the derived asset-tooling quality report.

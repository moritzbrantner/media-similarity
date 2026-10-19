# Native video evidence acceptance

Issue [#88](https://github.com/moritzbrantner/media-similarity/issues/88)
selects video OCR/evidence through Media Intelligence, with Tesseract retained as
its engine. [ADR 0008](../adr/0008-video-evidence-composition.md) describes the
implementation and public dependency contract.

## Baseline and tested artifact

The independent baseline at `f2d8ec2` (executed at `9bd3f69`, whose only additional
change removed a Compose subnet) passed source-relative transcript slicing and
existing real-FFmpeg tests. Existing video acceptance used FakeQdrant with OCR
disabled; existing real-Qdrant coverage was image-only and browser tests mocked
responses. Those checks did not establish the requested OCR/evidence journey.

Independent final acceptance ran the native binary with SHA-256
`62987c775c8ce701464f80c5b74e8a3e79b528023ab22966dac67616fd9dfe49`, built from the
implementation and face-storage repair subsequently committed in `cc742df`.
The later merge `14775d7` incorporates the separately accepted metadata provider
and pinned frontend dependency updates. Its lint, locked backend tests, frontend
format/build, architecture guard and all 81 existing browser tests pass. The
native journey below used real services and models, without API/browser mocks.

Shared conventions resolved to source revision
`6cc05789dab39cee6758b288f83fb48b4ee6dbdb`.

## Native observations

- Two scenes in an 11-second source video carry source-relative OCR tracks:
  `BUDGET 2026` at 0–5.9167 seconds and `REVENUE 2027` at 6–10.9167 seconds.
  Each track retains four real frame observations while contributing its text
  once to compatibility OCR. The source content hash matches the actual file.
- Actual Whisper timed JFK speech at 0–11.008 seconds overlaps both scenes with
  the same speech identity, complete producer receipts and scene relations.
  Video upload, scene selection and separate audio upload work in Chromium.
- Actual CLIP self-search returns `budget.png` first, cosine above 0.999, pHash
  distance zero and no degraded model flag. Duplicate-only/exclude predicates,
  typed video/audio text filters, name/path, limit and sort controls work.
- Actual YuNet/SFace search returns the indexed Obama image and existing person
  identity. Qdrant stores a deterministic physical UUID while payload face and
  person identities remain unchanged. No similarity thresholds were relaxed.
- Saving and reopening a smart album retains its OCR matches. Thumbnails and
  full-source/scene playback load; source range requests return HTTP 206.
- Unchanged reindex returns indexed=0/already_indexed=4 with byte-equivalent
  media payloads. Native restart/reload preserves IDs, evidence and skip state.
- With a nonexistent OCR executable, native indexing still succeeds. Video OCR
  coverage is `failed` with an explicit unavailable-engine reason, no OCR tracks
  are fabricated, and speech remains completed. The real browser shows the
  failure while transcript queries still return the scenes; readiness identifies
  the OCR failure independently of healthy Qdrant/models.
- Deleting and rebuilding the disposable Qdrant collection indexes eight media
  points without failure. All media IDs/payloads/evidence and the physical face
  ID are identical; all nine points' named vectors match within 1e-6.

The runtime used actual Tesseract 5.5.0 with English tessdata, native Whisper
`openai/whisper-tiny.en` on CPU, and the configured CLIP/YuNet/SFace bundles.
ONNX Runtime 1.31.0 was supplied through `ORT_DYLIB_PATH`; model files alone do
not provide that shared library. No Python process implements the media path.

## Other checks and limits

The focused regressions cover default-video transcription, stable profiles when
an unrelated workflow capability is disabled, and physical face storage without
logical identity changes. The architecture guard rejects Media Intelligence
implementation imports and renamed aliases in the domain.

The separate `bun run build:pages` passes. Its browser-local WASM output does
not establish the native Qdrant-backed journey. The public quality-corpus check
and asset-tooling fixture/metric checks also pass; these are distinct from the
full model evaluation, whose result must be recorded separately.

## Reproduction

The independently authored [native acceptance drivers](../../scripts/acceptance/video-evidence/README.md)
provide fixture preparation, owned-service cleanup, real browser journeys and
payload/vector assertions. They are manual native acceptance, separate from the
mocked browser suite and the full public quality corpus. Parameterizing the
drivers does not constitute another full acceptance run.

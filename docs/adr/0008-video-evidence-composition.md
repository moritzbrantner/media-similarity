# Video evidence through Media Intelligence

Status: implementation of the approved #88 video OCR/evidence slice.

Media Similarity retains source configuration and identity, inventory, Qdrant
payloads/vectors, pHash, visual embeddings, faces, smart albums, thumbnails and
ranking policy. `workers/media/video_evidence.rs` adapts the public Media
Intelligence `analyze()` operation. The domain and HTTP layers use application-owned
result types and do not import composition implementation crates.

Each source video is analysed once per indexing attempt. Existing native audio
analysis supplies timed transcript segments to a speech producer adapter, without
running ASR a second time. Media Intelligence detects source scenes and invokes
the canonical Visual Analysis OCR producer for representative scene frames. Its
JSON-command backend invokes the same service executable in `--video-ocr-json`
mode, which runs the configured Tesseract engine with the existing PSM 11 and
2000-pixel image bound. This needs no Python, shell adapter, new service or model
download. The image/GIF/PDF OCR paths retain their existing adapter.

The canonical producer owns frame sampling, text tracks and scene/text/speech
relations. `OCR_MAX_FRAMES` remains an image/GIF/PDF setting; video sampling follows
the pinned producer's representative-frame policy. One text contribution per
distinct producer track feeds compatibility OCR text, never one contribution per
repeated frame. Observation times and frame indices remain available in
`video_evidence`, together with source identity/content hash, scene/evidence IDs,
producer identity/config/input hash and independent OCR/transcript confidence.
All times refer to the complete original video, including later scenes. No
visual cosine, pHash, OCR, text or speaker scores are combined by this adapter.

Failed OCR is stored as failed coverage with its reason, while independently
successful scene/visual analysis can still be indexed. Disabled modalities are
not requested. Untimed transcript segments are retained in existing audio
metadata but explicitly fail common speech coverage rather than inventing times.
The UI shows coverage, source-relative evidence and producer details. Older
payloads may omit the optional field and remain readable.

The default video workflow now includes audio analysis, so configured video
transcription reaches the shared composition. A deliberately saved workflow
without that processor keeps it disabled; use the existing workflow reset/edit
controls to adopt the new default. Indexing profile v7 rebuilds existing points
once. Payloads, planning and the ledger use the same configuration/workflow
fingerprint, so an unchanged folder skips OCR/ASR on subsequent indexing runs.
Deleting/rebuilding Qdrant still uses the existing source pipeline and stable
source/scene point identities.

Native acceptance also exposed the old face store sending logical coordinate IDs
as physical Qdrant IDs, which Qdrant rejects. The storage adapter now maps those
keys to deterministic UUIDs for face upserts/payload updates. Logical face IDs,
person links and payloads stay unchanged; previously valid UUID keys remain
unchanged. This restores native face retrieval without changing its scoring.

## Public dependency contract

Media Intelligence analysis/core are consumed at exact public Git revision
`c4e66234193f33a7206cb6ca0e5385281d806d9a`. Its documented unpublished semantic
leaf is pinned to Foundation `b0e26e4e671e5c43d659a2c67583ffdf5a9bbfa9`.
Four compatibility patches align registry consumers' video core/ingest and image
core/detection contracts with the facade's Visual Analysis revision
`f73969447916e5ac47f4559fc51ab47552f4f734`. This avoids mixing the old video-owned
error/text contracts with current Foundation contracts, and consumes the existing
YuNet adaptation to the model's declared fixed input shape. Published Foundation
0.1.1 leaves and transcript contracts 0.1.3 replace incompatible older locked
versions; unused whisper.cpp model helpers were removed. No sibling checkout,
authenticated Git fallback or publication is required.

## Native runtime prerequisite

Actual ONNX inference requires its native shared library in addition to model
bundles. Set `ORT_DYLIB_PATH` to an installed compatible library when the system
loader cannot find it. Acceptance exposed an existing `ort` rc.12 missing-library
initialization deadlock, also present with main's identical `ort` revision; a
standalone session reproduces it, and providing the native library resolves it.
This observation does not establish a change in CLIP/face ranking quality.

The separate Pages/WASM build remains browser-local and is not evidence of the
native Qdrant-backed indexing journey.

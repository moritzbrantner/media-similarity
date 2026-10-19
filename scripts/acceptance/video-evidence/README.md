# Independent native video-evidence acceptance

These are parameterized copies of the independently authored assertions exercised for issue88, not an added test framework. They use the real Rust/Axum binary, Qdrant, Chromium, tesseract, CLIP, YuNet/SFace, and Candle Whisper. No route mocks or fixed-output recognizers are used. Build/install the repository and its Chromium dependency first using the repository instructions. Run from the repository root, or set MEDIA_SIMILARITY_ROOT. Python/transient inspectors are deliberately excluded.

Set ACCEPTANCE_ROOT to a fresh disposable directory. Defaults are API18888 and Qdrant6333; set BIND_ADDR/ACCEPTANCE_URL and ACCEPTANCE_QDRANT_PORT/QDRANT_URL together when changing ports. ORT_DYLIB_PATH must name a working native ONNX Runtime library. OCR_COMMAND and optional TESSDATA_PREFIX select actual tesseract. CACHED_BUNDLE_ROOT optionally supplies existing CLIP/YuNet/SFace/Whisper bundles through symlinks; MODEL_HF_CACHE_DIR optionally reuses the public Hugging Face cache. The scripts contain no workstation/cache paths or secrets.

Fixtures: bundled BUDGET bitmap sha25616b7dbb9c62c28b674b8a9ae756a98d13821e6cba62b663b156a5dc70bf5a664; public whisper.cpp JFK WAV supplied as JFK_WAV (tested sha25659dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e); existing quality corpus Obama source/query (obtain through the repository's quality:download command). OBAMA_SOURCE_JPG/OBAMA_QUERY_JPG can override those asset locations. prepare-fixtures.sh copies these, renders REVENUE2027, and encodes an11-second two-scene video with real JFK speech.

Models used: Xenova/clip-vit-base-patch32 quantized ONNX; OpenCV YuNet and SFace; openai/whisper-tiny.en CPU; tesseract5.5.0/eng. Bundle provenance is available from the native model API. The original environment used ONNX Runtime1.31.0. The application default is deliberately overridden to tiny.en, not Whisper turbo.

```bash
export MEDIA_SIMILARITY_ROOT="$PWD"
export ACCEPTANCE_SCRIPTS="$PWD/scripts/acceptance/video-evidence"
export ACCEPTANCE_ROOT="${TMPDIR:-/tmp}/media88-native-acceptance-data"
# Set JFK_WAV, ORT_DYLIB_PATH, OCR_COMMAND/TESSDATA_PREFIX as needed.
bash "$ACCEPTANCE_SCRIPTS/prepare-fixtures.sh"
bash "$ACCEPTANCE_SCRIPTS/services.sh" # Keep this trap-owning process running.
# In another terminal with the same environment:
bash "$ACCEPTANCE_SCRIPTS/start-app.sh"
```

Set ACCEPTANCE_SCRIPTS to `"$PWD/scripts/acceptance/video-evidence"`. If bundles are absent, use the native Models panel or POST /api/models/{role}/download with an empty JSON object; roles are visual_embedding, face_detection, face_embedding, audio_transcription. The explicitly selected audio model is tiny.en. In the actual acceptance, tiny.en was materialized through /api/models/audio_transcription/download with {"model":"openai/whisper-tiny.en"}. Wait for model jobs to succeed and readiness to pass before indexing. Use a fresh workflow file; /api/workflows/reset restores the current public defaults when intentionally testing disposable old state.

```bash
bun "$ACCEPTANCE_SCRIPTS/browser.mjs" # Real Sources Save and Index Sources controls.
bun "$ACCEPTANCE_SCRIPTS/wait-index.mjs"
bun "$ACCEPTANCE_SCRIPTS/ui-album.mjs" # New, Preview, Save and reload Acceptance budget.
bun "$ACCEPTANCE_SCRIPTS/final-browser.mjs" # OCR/image/video/audio/face, sort/filter/limit/scene switch.
bun "$ACCEPTANCE_SCRIPTS/lifecycle.mjs" face-refresh
bun "$ACCEPTANCE_SCRIPTS/face-browser.mjs"
# Stop/restart ONLY the owned native process, retaining the disposable data:
bun "$ACCEPTANCE_SCRIPTS/lifecycle.mjs" reload
bun "$ACCEPTANCE_SCRIPTS/lifecycle.mjs" rebuild # Deletes only the dedicated acceptance collection.
bun "$ACCEPTANCE_SCRIPTS/check-snapshot.mjs"
bun "$ACCEPTANCE_SCRIPTS/query-invariants.mjs"
bun "$ACCEPTANCE_SCRIPTS/playback.mjs"
# Stop native process; restart start-app.sh with:
# ACCEPTANCE_OCR_COMMAND="$ACCEPTANCE_ROOT/nonexistent-tesseract"
bun "$ACCEPTANCE_SCRIPTS/lifecycle.mjs" degrade
bun "$ACCEPTANCE_SCRIPTS/ui-degraded.mjs"
# Stop owned native process; trigger the Compose owner's trap:
touch "$ACCEPTANCE_ROOT/stop"
```

The missing-engine contract accepts failed OR unavailable coverage with an explicit unavailable reason, empty OCR tracks, surviving completed speech, and visible browser diagnostics. It must not be mistaken for completed analysis finding no text. This reflects the approved issue contract; the exercised adapter reports failed because its command wrapper starts but the underlying engine is absent.

Artifacts go under ACCEPTANCE_ROOT. Rebuild compares every media payload/evidence/identity and all named vectors within1e-6, including the physical UUID face point. The source-time assertion checks OCR observations in both [0,6) and [6,11), one text contribution per scene from four real observations, actual source SHA-256, scene relations, speech receipts/source times, and served artifacts.

Executed evidence is recorded separately in the task's acceptance note. The original full browser run passed nonface paths before the final storage-only repair, then the final artifact passed the independent face/album journey, reload, full collection rebuild, source evidence, filters, playback, and missing-engine checks. Parameterization does not imply a fresh full suite execution. The portable degraded UI/query drivers were additionally rerun against the actual final disposable service.

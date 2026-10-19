#!/usr/bin/env bash
set -euo pipefail
acceptance_root=${ACCEPTANCE_ROOT:-/tmp/media88-native-acceptance-data}
repo_root=${MEDIA_SIMILARITY_ROOT:-$PWD}
: "${ORT_DYLIB_PATH:?Set ORT_DYLIB_PATH to a working native ONNX Runtime shared library}"
mkdir -p "$acceptance_root/sources" "$acceptance_root/bundles"
touch "$acceptance_root/media-sources.txt" "$acceptance_root/empty-seed.txt"
if [[ -n ${CACHED_BUNDLE_ROOT:-} ]]; then
  for name in xenova-clip-vit-base-patch32-onnx opencv-yunet-onnx opencv-sface-onnx openai_whisper-tiny.en; do
    if [[ -d "$CACHED_BUNDLE_ROOT/$name" && ! -e "$acceptance_root/bundles/$name" ]]; then
      ln -s "$CACHED_BUNDLE_ROOT/$name" "$acceptance_root/bundles/$name"
    fi
  done
fi
export BIND_ADDR=${BIND_ADDR:-127.0.0.1:18888} QDRANT_URL=${QDRANT_URL:-http://127.0.0.1:6333} QDRANT_COLLECTION=${QDRANT_COLLECTION:-media88_acceptance}
export SOURCE_IMAGE_DIR="$acceptance_root/sources" MEDIA_SOURCES_FILE="$acceptance_root/media-sources.txt" MEDIA_SOURCES_SEED_FILE="$acceptance_root/empty-seed.txt" STARTUP_INDEXING_ENABLED=false SOURCE_WATCHING_ENABLED=false
export THUMBNAIL_DIR="$acceptance_root/thumbnails" UPLOAD_DIR="$acceptance_root/uploads" INDEXING_LEDGER_FILE="$acceptance_root/ledger.json" PROCESSING_WORKFLOWS_FILE="$acceptance_root/workflows.json" VOICE_REGISTRY_PATH="$acceptance_root/voices.json" SMART_ALBUMS_FILE="$acceptance_root/albums.json"
export MODEL_BUNDLE_DIR="$acceptance_root/bundles" MODEL_HF_CACHE_DIR=${MODEL_HF_CACHE_DIR:-$acceptance_root/hf-cache}
export AUDIO_TRANSCRIPTION_ENABLED=true AUDIO_TRANSCRIPTION_MODEL=openai/whisper-tiny.en AUDIO_TRANSCRIPTION_DEVICE=cpu AUDIO_TRANSCRIPTION_THREADS=2 AUDIO_TRANSCRIPTION_AUTO_DOWNLOAD=false
export VISUAL_EMBEDDING_BACKEND=onnx VISUAL_EMBEDDING_ENABLED=true VECTOR_SIZE=512 VISUAL_EMBEDDING_VECTOR_SIZE=512 FACE_EMBEDDING_VECTOR_SIZE=128 FACE_ANALYSIS_ENABLED=true
export OCR_ENABLED=true OCR_COMMAND=${ACCEPTANCE_OCR_COMMAND:-${OCR_COMMAND:-tesseract}} OCR_LANGUAGE=eng OCR_MAX_FRAMES=6
export VIDEO_FRAME_STRIDE=12 VIDEO_MAX_FRAMES=6 IMAGE_EXTENSIONS=.png,.jpg,.mp4 AUDIO_EXTENSIONS=.wav
export FRONTEND_SERVING_ENABLED=true FRONTEND_DIST_DIR=${FRONTEND_DIST_DIR:-$repo_root/frontend/dist}
exec "${APP_BINARY:-$repo_root/backend/target/debug/image-similarity-service}"

# Media Similarity

This context describes the media indexing, similarity search, and identity
recognition language used by the service.

## Language

**Local static image source**:
A supported still image file discovered from a configured local media folder.
_Avoid_: source image

**Query upload**:
A user-supplied media file decoded for search but not permanently indexed as a
source item.
_Avoid_: uploaded source

**Media point**:
A searchable indexed media record with payload metadata and a visual vector.
_Avoid_: Qdrant point when discussing product behavior

**Visual vector**:
The embedding used for visual similarity search. It is distinct from pHash and
is the primary semantic ranking signal.
_Avoid_: image hash, pHash

**pHash distance**:
A perceptual-hash distance used as a near-duplicate signal. It is not the
general semantic ranking score.
_Avoid_: similarity score

**Person identity**:
A cluster ID assigned to faces believed to belong to the same person.
_Avoid_: face label, person label

**Face query**:
An uploaded image whose selected face is embedded and searched against indexed
face vectors.
_Avoid_: person upload

**Quality corpus**:
A public, reproducible media set used to measure search and recognition
behavior.
_Avoid_: demo corpus, private benchmark

**Quality gate**:
A command or report that evaluates model-backed behavior against the quality
corpus.
_Avoid_: benchmark when referring to acceptance checks

**Degraded mode**:
Results produced without the configured quality model active.
_Avoid_: normal fallback

**Native transcription pipeline**:
The Rust audio/video speech indexing path that transcodes media, runs the
app-managed ASR model bundle through the native Candle Whisper provider, and
stores transcript text on the existing media analysis payload used by text
search. Audio and video query uploads use the same pipeline semantics as
indexed media.
_Avoid_: Python WhisperX runtime, whisper.cpp path, transcript service

**ASR model bundle**:
The app-managed model files for the configured speech recognizer, defaulting to
`openai/whisper-large-v3-turbo`, reported through the same model readiness
language as other model roles. A missing or unusable bundle is a blocking setup
condition for enabled speech-bearing transcription, not degraded mode.
_Avoid_: Python model install, CPU fallback model, ad hoc model path

**Video transcript slice**:
The source-video-relative transcript segments attached to a video scene media
point after the source video audio is transcribed once. A slice contains only
segments that overlap the scene window, while segment times remain relative to
the full source video for later playback and alignment work.
_Avoid_: full-video transcript per scene, scene-relative transcript timestamps,
first-class transcript record

**Indexing plan**:
The decision about which source items are pending, already current, skipped, or
stale.
_Avoid_: scan result

**Payload index**:
A Qdrant field index used to make filtered media search efficient.
_Avoid_: vector index

## Native Transcription Scope

The native transcription PRD keeps the existing query upload, media point,
visual vector, degraded mode, and quality gate vocabulary. It does not introduce
Python WhisperX execution, CPU-first transcription behavior, diarization,
word-level alignment, transcript export, or separate first-class transcript
records.

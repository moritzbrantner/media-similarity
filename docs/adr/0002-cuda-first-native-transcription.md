# Use CUDA-first native transcription

We use the native Rust transcription pipeline with Candle Whisper ASR and the
app-managed `openai/whisper-large-v3-turbo` ASR model bundle as the speech
indexing path for audio, video, and query uploads. Enabled transcription for
speech-bearing media requires the configured CUDA runtime and model bundle to
be usable; failures are blocking indexing errors rather than degraded mode
successes.

This branch intentionally treats current default GitHub Rust CI as an
insufficient acceptance gate for CUDA inference. The quality gate for this
decision is CUDA-backed smoke or behavior verification on CUDA-capable
infrastructure, because GitHub-hosted CI does not currently provide the runtime
profile needed to prove native CUDA ASR behavior. Keeping CPU-first
transcription, Python WhisperX runtime execution, diarization/alignment,
transcript export, and first-class transcript records are out of scope for this
PRD.

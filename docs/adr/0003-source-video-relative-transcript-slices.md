# Store video transcripts as source-video-relative scene slices

We transcribe each source video audio track once, then attach transcript text
and segments only to the video scene media points whose scene windows overlap
those segments. Each attached video transcript slice keeps segment timestamps
relative to the full source video instead of shifting them to scene-relative
time.

This preserves the existing media point and text-search payload contract while
making transcript search return the relevant scene instead of every scene from
the same video. It also keeps future playback, alignment, and quality gate
checks anchored to absolute source media time. Full-video transcripts on every
scene, scene-relative transcript timestamps, separate first-class transcript
records, transcript export, diarization, and word-level alignment are out of
scope for this PRD.

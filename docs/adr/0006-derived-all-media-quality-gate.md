# Use derived all-media quality cases for media acceptance

We extend the public optional quality corpus from visual and face-only cases into a small all-media quality gate covering static images, GIFs, audio, video, PDFs, OCR, and CUDA-first ASR. Query assets are derived locally from public source assets with deterministic classical transformations so the gate tests parsing, indexing, and search robustness without checking generated media into the repository.

This remains outside mandatory pull-request CI because OCR, ffmpeg/poppler, Qdrant, model bundles, and CUDA-first ASR make the gate environment-sensitive.

# Image Similarity Embedder Spike

## Command

The diagnostic added in this change is:

```bash
cargo run --manifest-path backend/Cargo.toml --bin image_similarity_diagnostic -- --image-root sample-images/quality --manifest tests/fixtures/quality-corpus/manifest.json
```

If the public quality corpus is not present, generate it first:

```bash
bun run quality:download
```

Private false-positive pairs can be tested without committing private media:

```bash
cargo run --manifest-path backend/Cargo.toml --bin image_similarity_diagnostic -- --pairs experiments/image-similarity/private-pairs.local.json
```

The local pairs file is intentionally not required for committed results. Its schema is:

```json
{
  "pairs": [
    {
      "id": "false-positive-1",
      "expected": "different_person",
      "left": "/absolute/or/manifest-relative/query.jpg",
      "right": "/absolute/or/manifest-relative/result.jpg"
    }
  ]
}
```

## Model Status Summary

This workspace did not have model bundles cached under `data/models/bundles`, so the active visual path degraded to the legacy fallback and the face detector/embedder were inactive.

| role | configured | active | cached | detail |
| --- | --- | --- | --- | --- |
| visual_embedding | xenova-clip-vit-base-patch32-onnx | false | false | Model bundle `xenova-clip-vit-base-patch32-onnx` is not cached in data/models/bundles |
| face_detection | opencv-yunet-onnx | false | false | Model bundle `opencv-yunet-onnx` is not cached in data/models/bundles |
| face_embedding | opencv-sface-onnx | false | false | Model bundle `opencv-sface-onnx` is not cached in data/models/bundles |

## Results Table

| pair_id | expected | left | right | active_visual_model | active_visual_degraded | active_visual_cosine | legacy_color_cosine | face_model_cosine | notes |
| --- | --- | --- | --- | --- | --- | ---: | ---: | ---: | --- |
| barack-obama-face--same--barack-obama-president | same_person:barack-obama | sample-images/quality/queries/barack-obama-query.jpg | sample-images/quality/sources/barack-obama-president.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 1.000000 | 1.000000 |  | face models inactive |
| barack-obama-face--same--barack-obama-smile | same_person:barack-obama | sample-images/quality/queries/barack-obama-query.jpg | sample-images/quality/sources/barack-obama-smile.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.997552 | 0.997552 |  | face models inactive |
| barack-obama-face--different--grace-hopper-navy | different_person:barack-obama!=grace-hopper | sample-images/quality/queries/barack-obama-query.jpg | sample-images/quality/sources/grace-hopper-navy.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.991862 | 0.991862 |  | face models inactive |
| barack-obama-face--different--marie-curie-1900 | different_person:barack-obama!=marie-curie | sample-images/quality/queries/barack-obama-query.jpg | sample-images/quality/sources/marie-curie-1900.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.991526 | 0.991526 |  | face models inactive |
| grace-hopper-face--same--grace-hopper-navy | same_person:grace-hopper | sample-images/quality/queries/grace-hopper-query.jpg | sample-images/quality/sources/grace-hopper-navy.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 1.000000 | 1.000000 |  | face models inactive |
| grace-hopper-face--same--grace-hopper-covered | same_person:grace-hopper | sample-images/quality/queries/grace-hopper-query.jpg | sample-images/quality/sources/grace-hopper-covered.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.993640 | 0.993640 |  | face models inactive |
| grace-hopper-face--different--barack-obama-president | different_person:grace-hopper!=barack-obama | sample-images/quality/queries/grace-hopper-query.jpg | sample-images/quality/sources/barack-obama-president.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.991862 | 0.991862 |  | face models inactive |
| grace-hopper-face--different--marie-curie-1900 | different_person:grace-hopper!=marie-curie | sample-images/quality/queries/grace-hopper-query.jpg | sample-images/quality/sources/marie-curie-1900.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.997197 | 0.997197 |  | face models inactive |
| marie-curie-face--same--marie-curie-1900 | same_person:marie-curie | sample-images/quality/queries/marie-curie-query.jpg | sample-images/quality/sources/marie-curie-1900.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 1.000000 | 1.000000 |  | face models inactive |
| marie-curie-face--same--marie-curie-1903 | same_person:marie-curie | sample-images/quality/queries/marie-curie-query.jpg | sample-images/quality/sources/marie-curie-1903.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.999987 | 0.999987 |  | face models inactive |
| marie-curie-face--different--barack-obama-president | different_person:marie-curie!=barack-obama | sample-images/quality/queries/marie-curie-query.jpg | sample-images/quality/sources/barack-obama-president.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.991526 | 0.991526 |  | face models inactive |
| marie-curie-face--different--grace-hopper-navy | different_person:marie-curie!=grace-hopper | sample-images/quality/queries/marie-curie-query.jpg | sample-images/quality/sources/grace-hopper-navy.jpg | legacy-fallback:sentence-transformers/clip-ViT-B-32 | true | 0.997197 | 0.997197 |  | face models inactive |

## Interpretation Criteria

- The active visual path is degraded in this workspace, so `active_visual_cosine` equals `legacy_color_cosine`.
- Different-person pairs score as high as `0.997197` under legacy color embeddings. This directly confirms that the fallback embedder can produce artificially high similarity for unrelated people.
- Face model separation could not be measured because YuNet and SFace bundles were not cached.
- The result still strongly supports separating identity search from broad visual/contextual search: degraded visual embeddings are unusable for identity, and even healthy CLIP-style full-frame embeddings should not be treated as face identity evidence.

## Spike Conclusion

The spike confirms that legacy fallback is directly capable of the reported near-collinear false positives. Different-person portrait pairs reached `0.991526-0.997197` cosine similarity when the active visual path degraded to the legacy color embedder.

The next spike run should download/cache the visual, face detection, and face embedding model bundles, then rerun the same command to fill in healthy CLIP and SFace separation. The PRD assumes separate identity and visual/contextual paths, with observability added before rollout so degraded vector generation can no longer hide.

# Use a public optional quality corpus for media similarity acceptance

We use a public, model-backed quality corpus as the acceptance standard for visual similarity and face recognition, but keep it outside mandatory pull-request CI at first. This keeps the normal CI path fast and deterministic while giving agents and humans a reproducible gate for changes that affect ranking, embeddings, face detection, or person matching; private-library-specific tuning remains out of scope for acceptance.

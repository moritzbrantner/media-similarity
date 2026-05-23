# Sample Corpus

This directory contains the manifest for the internet-backed sample corpus used
for local demos and sample-corpus checks.

The media files are intentionally not checked in. Generate them under the
ignored `sample-images/showcase` directory:

```bash
bun run sample:download
```

The manifest uses freely licensed or public sample files from Wikimedia
Commons, Blender's Big Buck Bunny project, and W3C. The downloader writes an
`ATTRIBUTION.md` file next to the generated media.

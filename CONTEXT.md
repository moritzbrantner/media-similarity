# Context

- **Local static image source**: A supported still image file discovered from a configured local media folder.
- **Query upload**: A user-supplied media file decoded for search but not permanently indexed as a source item.
- **Media point**: A searchable indexed media record with payload metadata and a visual vector.
- **Visual vector**: The embedding used for image similarity search.
- **Indexing plan**: The decision about which source items are pending, already current, skipped, or stale.
- **Payload index**: A Qdrant field index used to make filtered media search efficient.

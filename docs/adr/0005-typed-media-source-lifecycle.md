# Use typed media source lifecycle over text compatibility

We keep `media-sources.txt` as the user-editable compatibility surface, but
parse each configured source into a typed media source with a deterministic
source ID, normalized URI, capabilities, diagnostics, and preview inventory.

The source configuration API accepts syntactically valid but currently
unavailable sources. Unavailability, missing object-store credentials,
unsupported provider kinds, empty inventories, and inactive model-backed
features are reported as structured diagnostics instead of being collapsed into
one status string. The pre-index validation path is a bounded, non-mutating
source preview.

Indexing remains source-by-source fault isolated. A missing model bundle
degrades the affected feature, such as transcription or face analysis, rather
than blocking source saving or all possible indexing work for unrelated media.

Alternatives considered:

- Keeping plain source strings only. This preserved the smallest storage shape
  but left parsing, readiness, indexing, and UI semantics free to diverge.
- Migrating immediately to a JSON registry. This would preserve generated IDs
  across source edits, but it adds a migration and makes local hand editing less
  convenient before the source lifecycle has stabilized.
- Rejecting unavailable sources. This keeps configuration clean but makes
  removable drives, offline object stores, and staged setup painful.
- Globally blocking indexing on missing enabled models. This gives a simple
  readiness rule but prevents useful partial indexing and hides which feature is
  actually unavailable.

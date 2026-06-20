use crate::workers::sources::SourceImage;

use super::IndexedSourceRecord;

pub fn source_signature_matches(
    source_item_uri: &str,
    size_bytes: u64,
    modified_at: f64,
    indexing_profile: &str,
    source_image: &SourceImage,
    expected_indexing_profile: &str,
) -> bool {
    source_item_uri == source_image.item_uri
        && size_bytes == source_image.size_bytes
        && (modified_at - source_image.modified_at).abs() <= 0.001
        && indexing_profile == expected_indexing_profile
}

pub fn record_is_current(
    record: &IndexedSourceRecord,
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    record.size_bytes == source_image.size_bytes
        && (record.modified_at - source_image.modified_at).abs() <= 0.001
        && record.indexing_profile.as_deref() == Some(indexing_profile)
        && record.analysis_complete
}

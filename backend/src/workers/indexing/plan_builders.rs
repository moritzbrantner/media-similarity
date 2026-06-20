use crate::workers::sources::SourceImage;

use super::{record_is_current, IndexedSourceRecord};

pub fn source_is_current(
    indexed_records: &[IndexedSourceRecord],
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    indexed_records
        .iter()
        .any(|record| record_is_current(record, source_image, indexing_profile))
}

pub fn committed_records_are_current(
    indexed_records: &[IndexedSourceRecord],
    committed_point_ids: &[String],
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    !committed_point_ids.is_empty()
        && committed_point_ids.iter().all(|point_id| {
            indexed_records.iter().any(|record| {
                record.point_id == *point_id
                    && record_is_current(record, source_image, indexing_profile)
            })
        })
}

//! Rebuildable metadata projection. Qdrant payloads remain authoritative.
use std::collections::BTreeSet;
use std::path::Path;

use search_engine::indexed::{Mutation, TantivyIndex};
use search_engine::{Document, FieldType, Predicate, Schema, SearchRequest, Value};

use super::{MediaSearchFilter, StoredPoint};
use crate::domain::models::ImagePayload;

const PAGE_SIZE: usize = 1_000;
const NAME_FIELDS: [&str; 4] = ["filename", "relative_path", "path", "source_uri"];
const CAMERA_FIELDS: [&str; 3] = ["camera_make", "camera_model", "lens_model"];

pub(super) struct MetadataIndex {
    index: TantivyIndex,
}

impl MetadataIndex {
    pub(super) fn open(path: &Path) -> Result<Self, String> {
        let mut schema = Schema::new();
        for field in [
            "media_id",
            "source_type",
            "media_kind",
            "source_id",
            "source_item_uri",
        ] {
            schema = schema.field(field, FieldType::Keyword);
        }
        for field in NAME_FIELDS.into_iter().chain(CAMERA_FIELDS).chain(["text"]) {
            schema = schema.field(field, FieldType::Text);
        }
        for field in [
            "width",
            "height",
            "size_bytes",
            "modified_at",
            "captured_at",
        ] {
            schema = schema.field(field, FieldType::Integer);
        }
        schema = schema.field("has_gps", FieldType::Boolean);
        Ok(Self {
            index: TantivyIndex::open(path, schema).map_err(|error| error.to_string())?,
        })
    }

    fn next_revision(&self, id: &str) -> Result<u64, String> {
        self.index
            .latest_revision(id)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| format!("metadata projection revision exhausted for {id}"))
    }

    fn upsert_change(&self, id: &str, payload: &ImagePayload) -> Result<Option<Mutation>, String> {
        let mut document = project(id, payload, 0);
        if self
            .index
            .document(id)
            .is_some_and(|previous| previous.fields == document.fields)
        {
            return Ok(None);
        }
        document.revision = self.next_revision(id)?;
        Ok(Some(Mutation::Upsert { document }))
    }

    pub(super) fn upsert(&mut self, payload: &ImagePayload) -> Result<(), String> {
        let changes = self
            .upsert_change(&payload.id, payload)?
            .into_iter()
            .collect::<Vec<_>>();
        self.index
            .apply_batch(&changes)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(super) fn delete(&mut self, ids: &[String]) -> Result<(), String> {
        let mut changes = Vec::new();
        for id in ids {
            if self.index.document(id).is_some() {
                changes.push(Mutation::Delete {
                    id: id.clone(),
                    revision: self.next_revision(id)?,
                });
            }
        }
        self.index
            .apply_batch(&changes)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(super) fn reconcile(&mut self, snapshot: &[StoredPoint]) -> Result<(), String> {
        let previous_ids = self.ids(&MediaSearchFilter::default())?;
        let mut current_ids = BTreeSet::new();
        let mut changes = Vec::new();
        for point in snapshot {
            let Some(payload) = &point.payload else {
                continue;
            };
            let payload: ImagePayload =
                serde_json::from_value(payload.clone()).map_err(|error| {
                    format!("invalid authoritative media payload {}: {error}", point.id)
                })?;
            current_ids.insert(point.id.clone());
            changes.extend(self.upsert_change(&point.id, &payload)?);
        }
        for id in previous_ids {
            if !current_ids.contains(&id) {
                changes.push(Mutation::Delete {
                    revision: self.next_revision(&id)?,
                    id,
                });
            }
        }
        self.index
            .apply_batch(&changes)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(super) fn ids(&self, filter: &MediaSearchFilter) -> Result<Vec<String>, String> {
        let predicate = predicate(filter)?;
        let mut ids = Vec::new();
        loop {
            let page = self
                .index
                .search(&SearchRequest::new(predicate.clone()).page(ids.len(), PAGE_SIZE))
                .map_err(|error| format!("metadata candidate lookup failed: {error}"))?;
            if page.hits.is_empty() {
                break;
            }
            ids.extend(page.hits.into_iter().map(|hit| hit.id));
            if ids.len() >= page.total {
                break;
            }
        }
        Ok(ids)
    }
}

fn project(id: &str, payload: &ImagePayload, revision: u64) -> Document {
    let mut document = Document::new(id, revision)
        .field("media_id", Value::Keyword(id.to_owned()))
        .field("source_type", Value::Keyword(payload.source_type.clone()))
        .field("media_kind", Value::Keyword(payload.media_kind.clone()))
        .field("filename", Value::Text(payload.filename.clone()))
        .field("relative_path", Value::Text(payload.relative_path.clone()))
        .field("path", Value::Text(payload.path.clone()))
        .field(
            "source_uri",
            Value::Text(payload.source_uri.clone().unwrap_or_default()),
        )
        .field("width", Value::Integer(i64::from(payload.width)))
        .field("height", Value::Integer(i64::from(payload.height)))
        .field(
            "size_bytes",
            Value::Integer(unsigned_key(payload.size_bytes)),
        )
        .field(
            "has_gps",
            Value::Boolean(
                payload
                    .photo_metadata
                    .as_ref()
                    .is_some_and(|photo| photo.gps.is_some()),
            ),
        );
    for (field, value) in [
        ("source_id", &payload.source_id),
        ("source_item_uri", &payload.source_item_uri),
    ] {
        if let Some(value) = value {
            document = document.field(field, Value::Keyword(value.clone()));
        }
    }
    if let Some(key) = float_key(payload.modified_at) {
        document = document.field("modified_at", Value::Integer(key));
    }
    if let Some(photo) = &payload.photo_metadata {
        for (field, value) in CAMERA_FIELDS.into_iter().zip([
            &photo.camera_make,
            &photo.camera_model,
            &photo.lens_model,
        ]) {
            document = document.field(field, Value::Text(value.clone().unwrap_or_default()));
        }
        if let Some(key) = photo
            .capture_time
            .as_deref()
            .and_then(super::qdrant::parse_capture_time_epoch)
            .and_then(float_key)
        {
            document = document.field("captured_at", Value::Integer(key));
        }
    }
    let mut text = vec![payload.ocr_text.as_str()];
    text.extend(payload.ocr_frames.iter().map(|frame| frame.text.as_str()));
    if let Some(audio) = &payload.audio_analysis {
        text.push(&audio.transcript_text);
        text.extend(
            audio
                .transcript_segments
                .iter()
                .map(|segment| segment.text.as_str()),
        );
    }
    document.field("text", Value::Text(text.join(" ")))
}

// Preserve the full unsigned size range and the ordering of finite epoch seconds.
// In particular, dates are not rounded to milliseconds or filled from another date.
fn unsigned_key(value: u64) -> i64 {
    (value ^ (1_u64 << 63)) as i64
}
fn float_key(value: f64) -> Option<i64> {
    if !value.is_finite() {
        return None;
    }
    let bits = if value == 0.0 {
        0
    } else {
        value.to_bits() as i64
    };
    Some(bits ^ (((bits >> 63) as u64 >> 1) as i64))
}

fn predicate(filter: &MediaSearchFilter) -> Result<Predicate, String> {
    let mut predicates = Vec::new();
    for (field, value) in [
        ("source_type", &filter.source_type),
        ("media_kind", &filter.media_kind),
    ] {
        if let Some(value) = value {
            predicates.push(Predicate::Equals(
                field.into(),
                Value::Keyword(value.clone()),
            ));
        }
    }
    if let Some(ids) = &filter.ids {
        predicates.push(Predicate::Or(
            ids.iter()
                .map(|id| Predicate::Equals("media_id".into(), Value::Keyword(id.clone())))
                .collect(),
        ));
    }
    if let Some(value) = filter.has_gps {
        predicates.push(Predicate::Equals("has_gps".into(), Value::Boolean(value)));
    }
    for (field, min, max) in [
        (
            "width",
            filter.min_width.map(i64::from),
            filter.max_width.map(i64::from),
        ),
        (
            "height",
            filter.min_height.map(i64::from),
            filter.max_height.map(i64::from),
        ),
        (
            "size_bytes",
            filter.min_size_bytes.map(unsigned_key),
            filter.max_size_bytes.map(unsigned_key),
        ),
    ] {
        push_range(&mut predicates, field, min, max);
    }
    for (field, min, max) in [
        ("modified_at", filter.modified_from, filter.modified_to),
        ("captured_at", filter.captured_from, filter.captured_to),
    ] {
        let key =
            |value: f64| float_key(value).ok_or_else(|| format!("{field} filter must be finite"));
        push_range(
            &mut predicates,
            field,
            min.map(key).transpose()?,
            max.map(key).transpose()?,
        );
    }
    if let Some(query) = &filter.name_query {
        predicates.push(any_text(&NAME_FIELDS, query));
    }
    if let Some(query) = &filter.camera_query {
        predicates.push(any_text(&CAMERA_FIELDS, query));
    }
    if let Some(query) = &filter.text_query {
        predicates.extend(
            query
                .split_whitespace()
                .map(|term| Predicate::TextContains("text".into(), term.to_owned())),
        );
    }
    Ok(if predicates.is_empty() {
        Predicate::All
    } else {
        Predicate::And(predicates)
    })
}

fn any_text(fields: &[&str], query: &str) -> Predicate {
    Predicate::Or(
        fields
            .iter()
            .map(|field| Predicate::TextContains((*field).into(), query.to_owned()))
            .collect(),
    )
}
fn push_range(predicates: &mut Vec<Predicate>, field: &str, min: Option<i64>, max: Option<i64>) {
    if min.is_some() || max.is_some() {
        predicates.push(Predicate::Range {
            field: field.into(),
            minimum: min.map(Value::Integer),
            maximum: max.map(Value::Integer),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn media(id: &str, width: u32, modified_at: f64, captured: Option<&str>) -> ImagePayload {
        serde_json::from_value(json!({
            "id": id, "path": format!("/source/{id}.jpg"),
            "relative_path": format!("{id}.jpg"), "filename": format!("{id}.jpg"),
            "width": width, "height": 40, "size_bytes": 123,
            "modified_at": modified_at, "phash": "0000000000000000", "thumbnail_url": null,
            "source_uri": "file:///source", "ocr_text": "Invoice Total",
            "photo_metadata": captured.map(|date| json!({"capture_time": date, "camera_make": "Canon"}))
        })).unwrap()
    }

    fn points(media: &[ImagePayload]) -> Vec<StoredPoint> {
        media
            .iter()
            .map(|payload| StoredPoint {
                id: payload.id.clone(),
                payload: Some(serde_json::to_value(payload).unwrap()),
            })
            .collect()
    }

    #[test]
    fn typed_and_text_candidates_preserve_missing_dates_and_fractional_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let mut index = MetadataIndex::open(dir.path()).unwrap();
        index
            .reconcile(&points(&[
                media("dated", 80, 1.000_000_1, Some("2024-01-01T00:00:00Z")),
                media("missing", 80, 1.000_000_2, None),
                media("small", 20, 1.000_000_2, Some("2024-01-01T00:00:00Z")),
            ]))
            .unwrap();
        assert_eq!(
            index
                .ids(&MediaSearchFilter {
                    source_type: Some("local".into()),
                    media_kind: Some("static_image".into()),
                    min_width: Some(80),
                    max_height: Some(40),
                    min_size_bytes: Some(123),
                    max_size_bytes: Some(123),
                    name_query: Some("DATED.JPG".into()),
                    camera_query: Some("CANON".into()),
                    captured_from: Some(1_704_067_200.0),
                    captured_to: Some(1_704_067_200.0),
                    text_query: Some("invoice total".into()),
                    ..Default::default()
                })
                .unwrap(),
            ["dated"]
        );
        assert_eq!(
            index
                .ids(&MediaSearchFilter {
                    min_width: Some(80),
                    modified_from: Some(1.000_000_15),
                    ..Default::default()
                })
                .unwrap(),
            ["missing"]
        );
        assert_eq!(
            index
                .ids(&MediaSearchFilter {
                    min_width: Some(80),
                    captured_from: Some(0.0),
                    ..Default::default()
                })
                .unwrap(),
            ["dated"]
        );
        assert!(index
            .ids(&MediaSearchFilter {
                ids: Some(Vec::new()),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
    }

    #[test]
    fn reconciliation_and_reopen_preserve_revisions_and_prune_stale_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut index = MetadataIndex::open(dir.path()).unwrap();
        let initial = media("source", 80, 1.0, Some("2024-01-01T00:00:00Z"));
        index
            .reconcile(&points(std::slice::from_ref(&initial)))
            .unwrap();
        let revision = index.index.latest_revision("source").unwrap();
        index
            .reconcile(&points(std::slice::from_ref(&initial)))
            .unwrap();
        index.upsert(&initial).unwrap();
        assert_eq!(index.index.latest_revision("source"), Some(revision));
        drop(index);

        let mut index = MetadataIndex::open(dir.path()).unwrap();
        let updated = media("source", 20, 2.0, None);
        index.reconcile(&points(&[updated])).unwrap();
        assert!(index
            .ids(&MediaSearchFilter {
                min_width: Some(80),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
        assert!(index
            .ids(&MediaSearchFilter {
                captured_from: Some(0.0),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
        index.reconcile(&[]).unwrap();
        let deletion_revision = index.index.latest_revision("source").unwrap();
        assert!(deletion_revision > revision);
        drop(index);

        let mut index = MetadataIndex::open(dir.path()).unwrap();
        index.reconcile(&points(&[initial])).unwrap();
        assert_eq!(
            index.ids(&MediaSearchFilter::default()).unwrap(),
            ["source"]
        );
        assert!(index.index.latest_revision("source").unwrap() > deletion_revision);
    }

    #[test]
    fn candidate_lookup_pages_beyond_one_thousand_records() {
        let dir = tempfile::tempdir().unwrap();
        let mut index = MetadataIndex::open(dir.path()).unwrap();
        let records = (0..1_007)
            .map(|n| media(&format!("source-{n:04}"), 80, 1.0, None))
            .collect::<Vec<_>>();
        index.reconcile(&points(&records)).unwrap();
        assert_eq!(
            index.ids(&MediaSearchFilter::default()).unwrap().len(),
            records.len()
        );
    }

    #[test]
    fn numeric_keys_keep_finite_date_and_unsigned_size_order() {
        let dates = [-1e20, -1.5, -0.0, 0.0, 1e-12, 0.25, 1e20];
        assert!(dates
            .windows(2)
            .all(|pair| float_key(pair[0]) <= float_key(pair[1])));
        assert_eq!(float_key(-0.0), float_key(0.0));
        assert_eq!(float_key(f64::NAN), None);
        let sizes = [0, 1, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX];
        assert!(sizes
            .windows(2)
            .all(|pair| unsigned_key(pair[0]) < unsigned_key(pair[1])));
    }
}

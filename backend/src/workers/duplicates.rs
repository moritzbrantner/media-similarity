use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use crate::config::Settings;
use crate::domain::models::ImagePayload;
use crate::domain::smart_albums::DuplicateGroupSummary;
use crate::storage::MediaVectorStore;
use crate::workers::media::hashing::hash_distance;

const DUPLICATE_NAMESPACE: Uuid = Uuid::from_u128(0x7b9d0c8e_8117_4f5d_8f67_4ce53f9e029d);
pub const DUPLICATE_GROUP_RECORD_CAP: usize = 20_000;

#[derive(Clone, Debug, Default)]
pub struct DuplicateIndex {
    pub groups: Vec<DuplicateGroupSummary>,
    pub by_media_id: HashMap<String, DuplicateMembership>,
    pub warnings: Vec<String>,
    pub skipped_for_size: bool,
}

#[derive(Clone, Debug)]
pub struct DuplicateMembership {
    pub group_id: String,
    pub group_size: usize,
}

pub async fn duplicate_index_for_store(
    settings: &Settings,
    store: &dyn MediaVectorStore,
    require_duplicates: bool,
) -> Result<DuplicateIndex, String> {
    let points = store.scroll_media_points().await?;
    let total = points.len();
    if total > DUPLICATE_GROUP_RECORD_CAP {
        let detail = format!(
            "Corpus duplicate grouping is capped at {DUPLICATE_GROUP_RECORD_CAP} media records; collection has {total}"
        );
        if require_duplicates {
            return Err(detail);
        }
        return Ok(DuplicateIndex {
            warnings: vec![detail],
            skipped_for_size: true,
            ..DuplicateIndex::default()
        });
    }

    let media = points
        .into_iter()
        .filter_map(|point| point.payload)
        .filter_map(|payload| serde_json::from_value::<ImagePayload>(payload).ok())
        .filter(|payload| valid_phash(&payload.phash))
        .collect::<Vec<_>>();
    Ok(duplicate_index(settings.duplicate_hash_distance, &media))
}

pub fn duplicate_index(threshold: u32, media: &[ImagePayload]) -> DuplicateIndex {
    let mut parents = (0..media.len()).collect::<Vec<_>>();
    for left in 0..media.len() {
        for right in (left + 1)..media.len() {
            if hash_distance(&media[left].phash, &media[right].phash)
                .map(|distance| distance <= threshold)
                .unwrap_or(false)
            {
                union(&mut parents, left, right);
            }
        }
    }

    let mut grouped = BTreeMap::<usize, Vec<String>>::new();
    for (index, item) in media.iter().enumerate() {
        grouped
            .entry(find(&mut parents, index))
            .or_default()
            .push(item.id.clone());
    }

    let mut groups = grouped
        .into_values()
        .filter(|ids| ids.len() > 1)
        .map(|mut media_ids| {
            media_ids.sort();
            let id = duplicate_group_id(&media_ids);
            DuplicateGroupSummary {
                id,
                size: media_ids.len(),
                representative_media_id: media_ids[0].clone(),
                media_ids,
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        right.size.cmp(&left.size).then_with(|| {
            left.representative_media_id
                .cmp(&right.representative_media_id)
        })
    });

    let mut by_media_id = HashMap::new();
    for group in &groups {
        for media_id in &group.media_ids {
            by_media_id.insert(
                media_id.clone(),
                DuplicateMembership {
                    group_id: group.id.clone(),
                    group_size: group.size,
                },
            );
        }
    }

    DuplicateIndex {
        groups,
        by_media_id,
        warnings: Vec::new(),
        skipped_for_size: false,
    }
}

fn valid_phash(value: &str) -> bool {
    value.len() == 16 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn duplicate_group_id(media_ids: &[String]) -> String {
    let joined = media_ids.join("\n");
    let uuid = Uuid::new_v5(&DUPLICATE_NAMESPACE, joined.as_bytes())
        .simple()
        .to_string();
    format!("duplicate-{}", &uuid[..12])
}

fn find(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index {
        parents[index] = find(parents, parents[index]);
    }
    parents[index]
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left_root = find(parents, left);
    let right_root = find(parents, right);
    if left_root != right_root {
        parents[right_root] = left_root;
    }
}

#[cfg(test)]
mod tests {
    use super::duplicate_index;
    use crate::domain::models::ImagePayload;

    #[test]
    fn duplicate_grouping_unions_transitive_sets_and_ids_are_stable() {
        let media = vec![
            media("b", "0000000000000001"),
            media("a", "0000000000000000"),
            media("c", "0000000000000003"),
            media("d", "ffffffffffffffff"),
        ];

        let index = duplicate_index(1, &media);

        assert_eq!(index.groups.len(), 1);
        assert_eq!(index.groups[0].media_ids, vec!["a", "b", "c"]);
        assert_eq!(index.by_media_id["a"].group_size, 3);

        let reversed = media.into_iter().rev().collect::<Vec<_>>();
        let second = duplicate_index(1, &reversed);
        assert_eq!(index.groups[0].id, second.groups[0].id);
    }

    fn media(id: &str, phash: &str) -> ImagePayload {
        ImagePayload {
            id: id.to_string(),
            path: id.to_string(),
            relative_path: id.to_string(),
            filename: id.to_string(),
            width: 1,
            height: 1,
            size_bytes: 1,
            modified_at: 0.0,
            phash: phash.to_string(),
            thumbnail_url: None,
            animated_thumbnail_url: None,
            media_kind: "static_image".to_string(),
            frame_count: None,
            duration_ms: None,
            full_video_url: None,
            full_audio_url: None,
            full_pdf_url: None,
            pdf_page_url: None,
            pdf_document_id: None,
            pdf_page_index: None,
            pdf_page_number: None,
            pdf_page_count: None,
            audio_analysis: None,
            ocr_text: String::new(),
            ocr_frames: Vec::new(),
            visual_embedding_model: None,
            faces: Vec::new(),
            people: Vec::new(),
            artifacts: Vec::new(),
            tags: Vec::new(),
            photo_metadata: None,
            scene_clip_url: None,
            scene_index: None,
            scene_start_frame: None,
            scene_end_frame: None,
            scene_start_seconds: None,
            scene_end_seconds: None,
            source_type: "local".to_string(),
            source_item_uri: None,
            indexing_profile: None,
            source_uri: None,
        }
    }
}

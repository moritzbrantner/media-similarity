use crate::config::Settings;
use crate::domain::models::ImagePayload;
use crate::domain::smart_albums::{
    AlbumOrientationFilter, AlbumSortMode, DuplicateStatusFilter, SmartAlbum, SmartAlbumCriteria,
    SmartAlbumResult, SmartAlbumResultsResponse,
};
use crate::storage::{MediaSearchFilter, MediaVectorStore};
use crate::workers::duplicates::{duplicate_index_for_store, DuplicateIndex};

pub async fn smart_album_results(
    settings: &Settings,
    store: &dyn MediaVectorStore,
    album: SmartAlbum,
    offset: usize,
    limit: Option<u32>,
) -> Result<SmartAlbumResultsResponse, String> {
    store.ensure_collection().await?;
    let effective_limit = limit.unwrap_or(album.limit).clamp(1, 500);
    let duplicate_filter_active = album.criteria.duplicate_status != DuplicateStatusFilter::All;
    let duplicate_index =
        duplicate_index_for_store(settings, store, duplicate_filter_active).await?;
    let points = store
        .scroll_media_points_filtered(Some(qdrant_filter(&album.criteria)))
        .await?;
    let mut results = Vec::new();
    for point in points {
        let Some(payload) = point.payload else {
            continue;
        };
        let image = serde_json::from_value::<ImagePayload>(payload)
            .map_err(|error| format!("could not decode media payload: {error}"))?;
        if !matches_album_criteria(&image, &album.criteria, &duplicate_index) {
            continue;
        }
        let membership = duplicate_index.by_media_id.get(&image.id);
        results.push(SmartAlbumResult {
            image,
            duplicate_group_id: membership.map(|membership| membership.group_id.clone()),
            duplicate_group_size: membership
                .map(|membership| membership.group_size)
                .unwrap_or(1),
        });
    }
    sort_album_results(&mut results, album.sort);
    let total = results.len();
    let page = results
        .into_iter()
        .skip(offset)
        .take(effective_limit as usize)
        .collect::<Vec<_>>();

    Ok(SmartAlbumResultsResponse {
        album,
        count: page.len(),
        total,
        offset,
        limit: effective_limit,
        warnings: duplicate_index.warnings,
        duplicate_groups: duplicate_index.groups,
        results: page,
    })
}

fn qdrant_filter(criteria: &SmartAlbumCriteria) -> MediaSearchFilter {
    MediaSearchFilter {
        source_type: criteria.source_type.clone(),
        media_kind: criteria.media_kind.clone(),
        has_gps: criteria.has_gps,
        min_width: criteria.min_width,
        max_width: criteria.max_width,
        min_height: criteria.min_height,
        max_height: criteria.max_height,
        min_size_bytes: criteria.min_size_bytes,
        max_size_bytes: criteria.max_size_bytes,
        modified_from: criteria.modified_from,
        modified_to: criteria.modified_to,
        captured_from: criteria.captured_from,
        captured_to: criteria.captured_to,
    }
}

pub fn matches_album_criteria(
    image: &ImagePayload,
    criteria: &SmartAlbumCriteria,
    duplicate_index: &DuplicateIndex,
) -> bool {
    match criteria.duplicate_status {
        DuplicateStatusFilter::All => {}
        DuplicateStatusFilter::Only if !duplicate_index.by_media_id.contains_key(&image.id) => {
            return false;
        }
        DuplicateStatusFilter::Exclude if duplicate_index.by_media_id.contains_key(&image.id) => {
            return false;
        }
        _ => {}
    }

    if let Some(query) = criteria.name_query.as_deref() {
        if !matches_any_text(
            query,
            [
                image.filename.as_str(),
                image.relative_path.as_str(),
                image.path.as_str(),
                image.source_item_uri.as_deref().unwrap_or_default(),
                image.source_uri.as_deref().unwrap_or_default(),
            ],
        ) {
            return false;
        }
    }

    if let Some(query) = criteria.camera_query.as_deref() {
        let Some(metadata) = &image.photo_metadata else {
            return false;
        };
        if !matches_any_text(
            query,
            [
                metadata.camera_make.as_deref().unwrap_or_default(),
                metadata.camera_model.as_deref().unwrap_or_default(),
                metadata.lens_model.as_deref().unwrap_or_default(),
            ],
        ) {
            return false;
        }
    }

    if let Some(query) = criteria.keyword_query.as_deref() {
        let metadata_matches = image
            .photo_metadata
            .as_ref()
            .map(|metadata| {
                metadata
                    .keywords
                    .iter()
                    .any(|keyword| text_matches(keyword, query))
            })
            .unwrap_or(false);
        let tag_matches = image.tags.iter().any(|tag| text_matches(tag, query));
        if !metadata_matches && !tag_matches {
            return false;
        }
    }

    if let Some(query) = criteria.text_query.as_deref() {
        let ocr_frame_matches = image
            .ocr_frames
            .iter()
            .any(|frame| text_matches(&frame.text, query));
        let transcript_matches = image
            .audio_analysis
            .as_ref()
            .map(|analysis| {
                text_matches(&analysis.transcript_text, query)
                    || analysis
                        .transcript_segments
                        .iter()
                        .any(|segment| text_matches(&segment.text, query))
            })
            .unwrap_or(false);
        if !text_matches(&image.ocr_text, query) && !ocr_frame_matches && !transcript_matches {
            return false;
        }
    }

    if let Some(person_id) = criteria.person_id.as_deref() {
        if !image
            .people
            .iter()
            .any(|person| person.person_id == person_id)
        {
            return false;
        }
    }

    if let Some(speaker_id) = criteria.speaker_id.as_deref() {
        let Some(analysis) = &image.audio_analysis else {
            return false;
        };
        let recognized = analysis
            .recognized_voices
            .iter()
            .any(|voice| voice.id == speaker_id);
        let segmented = analysis
            .audio_segments
            .iter()
            .any(|segment| segment.speaker_id.as_deref() == Some(speaker_id));
        if !recognized && !segmented {
            return false;
        }
    }

    if criteria
        .orientation
        .is_some_and(|orientation| image_orientation(image.width, image.height) != orientation)
    {
        return false;
    }

    true
}

fn sort_album_results(results: &mut [SmartAlbumResult], sort: AlbumSortMode) {
    match sort {
        AlbumSortMode::CapturedNewest => results.sort_by(|left, right| {
            capture_time(&right.image)
                .total_cmp(&capture_time(&left.image))
                .then_with(|| left.image.filename.cmp(&right.image.filename))
        }),
        AlbumSortMode::DuplicateGroupSize => results.sort_by(|left, right| {
            right
                .duplicate_group_size
                .cmp(&left.duplicate_group_size)
                .then_with(|| left.image.filename.cmp(&right.image.filename))
        }),
        AlbumSortMode::Filename => results.sort_by(|left, right| {
            left.image
                .filename
                .to_lowercase()
                .cmp(&right.image.filename.to_lowercase())
        }),
        AlbumSortMode::ModifiedNewest => results.sort_by(|left, right| {
            right
                .image
                .modified_at
                .total_cmp(&left.image.modified_at)
                .then_with(|| left.image.filename.cmp(&right.image.filename))
        }),
        AlbumSortMode::SizeLargest => results.sort_by(|left, right| {
            right
                .image
                .size_bytes
                .cmp(&left.image.size_bytes)
                .then_with(|| left.image.filename.cmp(&right.image.filename))
        }),
    }
}

fn capture_time(image: &ImagePayload) -> f64 {
    image
        .photo_metadata
        .as_ref()
        .and_then(|metadata| metadata.capture_time.as_deref())
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp() as f64)
        .unwrap_or(0.0)
}

fn matches_any_text<'a>(query: &str, values: impl IntoIterator<Item = &'a str>) -> bool {
    values.into_iter().any(|value| text_matches(value, query))
}

fn text_matches(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(&query.to_lowercase())
}

fn image_orientation(width: u32, height: u32) -> AlbumOrientationFilter {
    if width == height {
        AlbumOrientationFilter::Square
    } else if width > height {
        AlbumOrientationFilter::Landscape
    } else {
        AlbumOrientationFilter::Portrait
    }
}

#[cfg(test)]
mod tests {
    use super::matches_album_criteria;
    use crate::domain::models::{
        AudioAnalysis, AudioSegmentGuess, AudioTranscriptSegment, ImagePayload,
    };
    use crate::domain::smart_albums::SmartAlbumCriteria;
    use crate::workers::duplicates::DuplicateIndex;

    #[test]
    fn text_matching_covers_ocr_and_transcript_segments() {
        let mut image = base_image();
        image.ocr_text = "Invoice total due".to_string();
        assert!(matches_album_criteria(
            &image,
            &SmartAlbumCriteria {
                text_query: Some("invoice".to_string()),
                ..SmartAlbumCriteria::default()
            },
            &DuplicateIndex::default(),
        ));

        image.ocr_text.clear();
        image.audio_analysis = Some(AudioAnalysis {
            speech_detected: true,
            speech_ratio: 1.0,
            speech_segments: Vec::new(),
            audio_segments: Vec::new(),
            recognized_voices: Vec::new(),
            transcript_text: String::new(),
            transcript_language: None,
            transcript_segments: vec![AudioTranscriptSegment {
                segment_index: 0,
                start_seconds: None,
                end_seconds: None,
                text: "hello archive".to_string(),
                confidence: None,
            }],
            tempo_bpm: None,
            tempo_confidence: 0.0,
            tempo_onset_count: 0,
        });
        assert!(matches_album_criteria(
            &image,
            &SmartAlbumCriteria {
                text_query: Some("archive".to_string()),
                ..SmartAlbumCriteria::default()
            },
            &DuplicateIndex::default(),
        ));
    }

    #[test]
    fn speaker_filter_matches_segments() {
        let mut image = base_image();
        image.audio_analysis = Some(AudioAnalysis {
            speech_detected: true,
            speech_ratio: 1.0,
            speech_segments: Vec::new(),
            audio_segments: vec![AudioSegmentGuess {
                segment_index: 0,
                kind: "speech".to_string(),
                start_seconds: 0.0,
                end_seconds: 1.0,
                confidence: 0.9,
                speaker_id: Some("voice-1".to_string()),
                speaker_label: Some("Voice".to_string()),
            }],
            recognized_voices: Vec::new(),
            transcript_text: String::new(),
            transcript_language: None,
            transcript_segments: Vec::new(),
            tempo_bpm: None,
            tempo_confidence: 0.0,
            tempo_onset_count: 0,
        });

        assert!(matches_album_criteria(
            &image,
            &SmartAlbumCriteria {
                speaker_id: Some("voice-1".to_string()),
                ..SmartAlbumCriteria::default()
            },
            &DuplicateIndex::default(),
        ));
    }

    fn base_image() -> ImagePayload {
        ImagePayload {
            id: "media".to_string(),
            path: "media".to_string(),
            relative_path: "media".to_string(),
            filename: "media".to_string(),
            width: 10,
            height: 8,
            size_bytes: 10,
            modified_at: 0.0,
            phash: "0000000000000000".to_string(),
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

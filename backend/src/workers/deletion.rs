use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Settings;
use crate::domain::models::ImagePayload;
use crate::storage::qdrant::{QdrantImageStore, StoredPoint};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DeleteIndexedSourceFilter {
    pub source_uri: Option<String>,
    pub source_item_uri: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DeleteIndexResponse {
    pub deleted_points: usize,
    pub deleted_faces: usize,
    pub deleted_artifacts: usize,
    pub errors: Vec<String>,
}

pub async fn delete_indexed_media(
    settings: &Settings,
    store: &QdrantImageStore,
    media_id: &str,
) -> DeleteIndexResponse {
    delete_matching_media(settings, store, Some(media_id), None, None).await
}

pub async fn delete_indexed_source(
    settings: &Settings,
    store: &QdrantImageStore,
    filter: DeleteIndexedSourceFilter,
) -> DeleteIndexResponse {
    delete_matching_media(
        settings,
        store,
        None,
        filter.source_uri.as_deref(),
        filter.source_item_uri.as_deref(),
    )
    .await
}

async fn delete_matching_media(
    settings: &Settings,
    store: &QdrantImageStore,
    id: Option<&str>,
    source_uri: Option<&str>,
    source_item_uri: Option<&str>,
) -> DeleteIndexResponse {
    let mut response = DeleteIndexResponse::default();
    let points = match store
        .scroll_media_points_by_filter(id, source_uri, source_item_uri)
        .await
    {
        Ok(points) => points,
        Err(error) => {
            response.errors.push(error);
            return response;
        }
    };
    let media_ids = points
        .iter()
        .map(|point| point.id.clone())
        .collect::<Vec<_>>();
    let face_points = match store.scroll_face_points_by_media_ids(&media_ids).await {
        Ok(points) => points,
        Err(error) => {
            response.errors.push(error);
            Vec::new()
        }
    };

    let artifact_paths = artifact_paths_for_points(settings, &points, &mut response.errors);
    let mut point_ids = media_ids;
    point_ids.extend(face_points.iter().map(|point| point.id.clone()));
    point_ids.sort();
    point_ids.dedup();

    match store.delete_points_by_ids(&point_ids).await {
        Ok(()) => {
            response.deleted_points = points.len();
            response.deleted_faces = face_points.len();
        }
        Err(error) => response.errors.push(error),
    }

    for path in artifact_paths {
        match fs::remove_file(&path) {
            Ok(()) => response.deleted_artifacts += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => response.errors.push(format!("{}: {error}", path.display())),
        }
        remove_empty_parents(&path, &[&settings.thumbnail_dir, &settings.upload_dir]);
    }

    response
}

fn artifact_paths_for_points(
    settings: &Settings,
    points: &[StoredPoint],
    errors: &mut Vec<String>,
) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    for point in points {
        let Some(payload_value) = &point.payload else {
            continue;
        };
        let payload = match serde_json::from_value::<ImagePayload>(payload_value.clone()) {
            Ok(payload) => payload,
            Err(error) => {
                errors.push(format!("{}: could not decode payload: {error}", point.id));
                continue;
            }
        };
        for url in payload
            .artifacts
            .iter()
            .map(|artifact| artifact.url.as_str())
            .chain(payload.thumbnail_url.as_deref())
            .chain(payload.animated_thumbnail_url.as_deref())
            .chain(payload.full_video_url.as_deref())
            .chain(payload.full_audio_url.as_deref())
            .chain(payload.full_pdf_url.as_deref())
            .chain(payload.pdf_page_url.as_deref())
            .chain(payload.scene_clip_url.as_deref())
        {
            match generated_url_to_path(settings, url) {
                Ok(Some(path)) => {
                    paths.insert(path);
                }
                Ok(None) => {}
                Err(error) => errors.push(error),
            }
        }
    }
    paths
}

pub fn generated_url_to_path(settings: &Settings, url: &str) -> Result<Option<PathBuf>, String> {
    let path_part = url.split_once('#').map_or(url, |(base, _)| base);
    if let Some(relative) = path_part.strip_prefix("/thumbnails/") {
        return safe_join(&settings.thumbnail_dir, relative).map(Some);
    }
    if let Some(relative) = path_part.strip_prefix("/uploads/") {
        return safe_join(&settings.upload_dir, relative).map(Some);
    }
    Ok(None)
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "refusing to delete generated artifact outside configured roots: {}",
            relative.display()
        ));
    }
    Ok(root.join(relative))
}

fn remove_empty_parents(path: &Path, roots: &[&Path]) {
    let Some(mut parent) = path.parent() else {
        return;
    };
    while roots.iter().any(|root| parent.starts_with(root)) && !roots.contains(&parent) {
        if fs::remove_dir(parent).is_err() {
            break;
        }
        let Some(next) = parent.parent() else {
            break;
        };
        parent = next;
    }
}

#[cfg(test)]
mod tests {
    use super::generated_url_to_path;
    use crate::config::Settings;

    #[test]
    fn generated_url_to_path_refuses_parent_segments() {
        let settings = Settings::default();
        let error = generated_url_to_path(&settings, "/uploads/../source.mp4").unwrap_err();
        assert!(error.contains("refusing to delete"));
    }
}

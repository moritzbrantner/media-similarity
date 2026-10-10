//! One-time backfill of source identity on legacy media points (owner decision on PR #66,
//! option B). Points written before `source_item_uri`/`source_uri` existed cannot be found by
//! the scoped watcher planner's exact filters; the backfill derives both fields like
//! `legacy_source_item_uri` and writes them. Until no legacy point is left, watcher indexing
//! falls back to a full rescan (see `ImageIndexer::plan_changed_local_sources`).

use std::path::{Path, PathBuf};

use crate::config::Settings;
use crate::domain::models::ImagePayload;
use crate::storage::MediaVectorStore;
use crate::workers::indexing::planner::legacy_source_item_uri;
use crate::workers::sources::build_image_sources;

/// Outcome of one backfill pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LegacySourceBackfill {
    /// Legacy points that now carry their source identity.
    pub updated: usize,
    /// Legacy points whose payload cannot be read; they are not retried.
    pub unreadable: usize,
    /// Legacy points whose write failed; the pass should be retried.
    pub failed_writes: usize,
}

/// True while media points without `source_item_uri` remain (reads at most one point).
pub async fn legacy_media_points_remain(store: &dyn MediaVectorStore) -> Result<bool, String> {
    Ok(!store.scroll_legacy_media_points(Some(1)).await?.is_empty())
}

/// Writes `source_item_uri` (and, when a configured source contains the item, `source_uri`) on
/// every legacy media point. Reads only legacy points; modern points are not touched.
pub async fn backfill_legacy_source_identity(
    store: &dyn MediaVectorStore,
    settings: &Settings,
) -> Result<LegacySourceBackfill, String> {
    let legacy_points = store.scroll_legacy_media_points(None).await?;
    if legacy_points.is_empty() {
        return Ok(LegacySourceBackfill::default());
    }
    let sources = build_image_sources(settings)
        .iter()
        .map(|source| {
            let root = source
                .local_root()
                .map(|root| root.canonicalize().unwrap_or_else(|_| root.to_path_buf()));
            (source.uri(), root)
        })
        .collect::<Vec<_>>();

    let mut outcome = LegacySourceBackfill::default();
    for point in legacy_points {
        let Some(payload) = point
            .payload
            .and_then(|payload| serde_json::from_value::<ImagePayload>(payload).ok())
        else {
            outcome.unreadable += 1;
            continue;
        };
        let Some(source_item_uri) = legacy_source_item_uri(&payload) else {
            outcome.unreadable += 1;
            continue;
        };
        // The configured sources decide; a stored `source_uri` (older than `source_item_uri`) is
        // kept only when no configured source contains the item.
        let source_uri = containing_source_uri(&sources, &source_item_uri).or(payload.source_uri);
        match store
            .set_legacy_media_source_identity(&point.id, &source_item_uri, source_uri.as_deref())
            .await
        {
            Ok(()) => outcome.updated += 1,
            Err(error) => {
                tracing::warn!(point_id = %point.id, %error, "could not backfill media source identity");
                outcome.failed_writes += 1;
            }
        }
    }
    Ok(outcome)
}

fn containing_source_uri(
    sources: &[(String, Option<PathBuf>)],
    source_item_uri: &str,
) -> Option<String> {
    let item = Path::new(source_item_uri);
    sources
        .iter()
        .filter(|(uri, root)| match root {
            Some(root) => item.starts_with(root),
            None => source_item_uri
                .strip_prefix(uri.as_str())
                .is_some_and(|rest| rest.starts_with('/')),
        })
        // The most specific configured source wins when roots are nested.
        .max_by_key(|(uri, root)| {
            root.as_ref()
                .map(|root| root.as_os_str().len())
                .unwrap_or(uri.len())
        })
        .map(|(uri, _)| uri.clone())
}

#[cfg(test)]
mod tests {
    use super::containing_source_uri;
    use std::path::PathBuf;

    #[test]
    fn picks_the_most_specific_containing_source() {
        let sources = vec![
            ("/media".to_string(), Some(PathBuf::from("/media"))),
            (
                "/media/albums".to_string(),
                Some(PathBuf::from("/media/albums")),
            ),
            ("s3://bucket/photos".to_string(), None),
        ];
        assert_eq!(
            containing_source_uri(&sources, "/media/albums/a.png").as_deref(),
            Some("/media/albums")
        );
        assert_eq!(
            containing_source_uri(&sources, "/media/b.png").as_deref(),
            Some("/media")
        );
        assert_eq!(
            containing_source_uri(&sources, "s3://bucket/photos/c.png").as_deref(),
            Some("s3://bucket/photos")
        );
        assert_eq!(containing_source_uri(&sources, "/elsewhere/d.png"), None);
        assert_eq!(containing_source_uri(&sources, "/media-other/e.png"), None);
        assert_eq!(
            containing_source_uri(&sources, "s3://bucket/photos2/f.png"),
            None
        );
    }
}

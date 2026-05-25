use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use jobs_core::{JobSnapshot, JobSpec};
use notify::event::{AccessKind, AccessMode, CreateKind, MetadataKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::{self, Instant, MissedTickBehavior};
use uuid::Uuid;

use crate::api::{run_index_job, AppState};
use crate::config::Settings;
use crate::workers::sources::{build_image_sources, video_extensions};

const WATCH_RESCAN_INTERVAL: Duration = Duration::from_secs(5);
const DEBOUNCE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const MAX_SAMPLE_PATHS: usize = 8;

pub fn spawn_local_source_watcher(state: Arc<AppState>) -> Option<tokio::task::JoinHandle<()>> {
    if !state.settings.source_watching_enabled {
        tracing::info!("local source file watching is disabled");
        return None;
    }

    Some(tokio::spawn(async move {
        if let Err(error) = watch_local_sources(state).await {
            tracing::warn!(%error, "local source file watcher stopped");
        }
    }))
}

async fn watch_local_sources(state: Arc<AppState>) -> Result<(), String> {
    let debounce = Duration::from_millis(state.settings.source_watching_debounce_ms);
    let (events_tx, mut events_rx) = mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = events_tx.send(event);
    })
    .map_err(|error| format!("could not start source watcher: {error}"))?;
    let mut watched_roots = BTreeSet::new();
    if let Err(error) = reconcile_watches(&mut watcher, &mut watched_roots, &state) {
        tracing::warn!(%error, "could not initialize all watched source directories");
    }

    let mut rescan = time::interval(WATCH_RESCAN_INTERVAL);
    rescan.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut debounce_tick = time::interval(DEBOUNCE_POLL_INTERVAL);
    debounce_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut pending_paths = BTreeSet::new();
    let mut last_event_at: Option<Instant> = None;

    loop {
        tokio::select! {
            maybe_event = events_rx.recv() => {
                let Some(event) = maybe_event else {
                    return Err("source watcher event channel closed".to_string());
                };
                match event {
                    Ok(event) => {
                        let settings = state.indexing_settings();
                        if event_may_affect_index(&event, &settings) {
                            for path in event.paths {
                                pending_paths.insert(path);
                            }
                            if pending_paths.is_empty() {
                                pending_paths.insert(PathBuf::from("<unknown>"));
                            }
                            last_event_at = Some(Instant::now());
                        }
                    }
                    Err(error) => tracing::warn!(%error, "source watcher event error"),
                }
            }
            _ = rescan.tick() => {
                if let Err(error) = reconcile_watches(&mut watcher, &mut watched_roots, &state) {
                    tracing::warn!(%error, "could not refresh watched source directories");
                }
            }
            _ = debounce_tick.tick() => {
                let Some(last_event) = last_event_at else {
                    continue;
                };
                if last_event.elapsed() < debounce {
                    continue;
                }
                if pending_paths.is_empty() {
                    last_event_at = None;
                    continue;
                }
                if index_job_is_active(&state) {
                    last_event_at = Some(Instant::now());
                    tracing::debug!("deferring source watcher index job while another index job is active");
                    continue;
                }

                match spawn_watch_index_job(&state, &pending_paths) {
                    Ok(snapshot) => {
                        tracing::info!(
                            job_id = %snapshot.spec.id,
                            changed_paths = pending_paths.len(),
                            "queued file watcher indexing job"
                        );
                        pending_paths.clear();
                        last_event_at = None;
                    }
                    Err(error) => {
                        tracing::warn!(%error, "could not queue file watcher indexing job");
                        last_event_at = Some(Instant::now());
                    }
                }
            }
        }
    }
}

fn reconcile_watches(
    watcher: &mut notify::RecommendedWatcher,
    watched_roots: &mut BTreeSet<PathBuf>,
    state: &AppState,
) -> Result<(), String> {
    let desired_roots = configured_local_source_roots(&state.indexing_settings());
    let removed = watched_roots
        .difference(&desired_roots)
        .cloned()
        .collect::<Vec<_>>();
    for root in removed {
        match watcher.unwatch(&root) {
            Ok(()) => tracing::info!(path = %root.display(), "stopped watching local source"),
            Err(error) => tracing::debug!(
                path = %root.display(),
                %error,
                "could not unwatch local source"
            ),
        }
        watched_roots.remove(&root);
    }

    let mut errors = Vec::new();
    let added = desired_roots
        .difference(watched_roots)
        .cloned()
        .collect::<Vec<_>>();
    for root in added {
        match watcher.watch(&root, RecursiveMode::Recursive) {
            Ok(()) => {
                tracing::info!(path = %root.display(), "watching local source for changes");
                watched_roots.insert(root);
            }
            Err(error) => {
                errors.push(format!("{}: {error}", root.display()));
            }
        }
    }

    if watched_roots.is_empty() {
        tracing::debug!("no configured local source directories are available to watch");
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "could not watch {} local source(s): {}",
            errors.len(),
            errors.join("; ")
        ))
    }
}

pub(crate) fn configured_local_source_roots(settings: &Settings) -> BTreeSet<PathBuf> {
    build_image_sources(settings)
        .into_iter()
        .filter_map(|source| {
            source
                .local_root()
                .and_then(|root| normalize_watch_root(root).ok())
        })
        .collect()
}

fn normalize_watch_root(root: &Path) -> Result<PathBuf, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    Ok(root.canonicalize().unwrap_or_else(|_| root.to_path_buf()))
}

fn index_job_is_active(state: &AppState) -> bool {
    match state.jobs.snapshots() {
        Ok(snapshots) => snapshots.iter().any(|snapshot| {
            snapshot
                .spec
                .kind
                .as_deref()
                .map(|kind| kind.starts_with("index."))
                .unwrap_or(false)
                && !snapshot.status.is_terminal()
        }),
        Err(error) => {
            tracing::warn!(%error, "could not inspect active index jobs");
            true
        }
    }
}

fn spawn_watch_index_job(
    state: &Arc<AppState>,
    changed_paths: &BTreeSet<PathBuf>,
) -> Result<JobSnapshot, String> {
    let settings = state.indexing_settings();
    let sample_paths = changed_paths
        .iter()
        .take(MAX_SAMPLE_PATHS)
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
    let spec = JobSpec::new(
        format!("index.watch.{}", Uuid::new_v4()),
        "Index changed media sources",
    )
    .and_then(|spec| spec.with_kind("index.watch"))
    .and_then(|spec| spec.with_metadata("collection", settings.qdrant_collection.clone()))
    .and_then(|spec| spec.with_metadata("trigger", "file_watch"))
    .and_then(|spec| spec.with_metadata("changed_paths", changed_paths.len().to_string()))
    .and_then(|spec| spec.with_metadata("sample_paths", sample_paths))
    .map_err(|error| error.to_string())?;
    let jobs = state.jobs.clone();
    let store = state.store.clone();
    let embedder = state.embedder.clone();

    jobs.spawn(spec, move |context| {
        run_index_job(context, settings, store, embedder)
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn event_may_affect_index(event: &Event, settings: &Settings) -> bool {
    if !event_kind_may_affect_index(event.kind) {
        return false;
    }
    if event.paths.is_empty() {
        return true;
    }

    let extensions = indexable_extensions(settings);
    event
        .paths
        .iter()
        .any(|path| path_may_affect_index(path, event.kind, &extensions))
}

fn event_kind_may_affect_index(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any
            | EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Any)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any))
            | EventKind::Modify(ModifyKind::Metadata(MetadataKind::WriteTime))
            | EventKind::Access(AccessKind::Close(AccessMode::Write))
    )
}

fn path_may_affect_index(path: &Path, kind: EventKind, extensions: &BTreeSet<String>) -> bool {
    if path_has_indexable_extension(path, extensions) {
        return true;
    }

    matches!(
        kind,
        EventKind::Create(CreateKind::Any | CreateKind::Folder)
            | EventKind::Remove(RemoveKind::Any | RemoveKind::Folder)
            | EventKind::Modify(ModifyKind::Name(_))
    ) && path.extension().is_none()
}

fn path_has_indexable_extension(path: &Path, extensions: &BTreeSet<String>) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
        .map(|extension| extensions.contains(&extension))
        .unwrap_or(false)
}

fn indexable_extensions(settings: &Settings) -> BTreeSet<String> {
    let mut extensions = settings.image_extensions.clone();
    extensions.extend(video_extensions());
    extensions.extend(settings.audio_extensions.clone());
    extensions.extend(settings.pdf_extensions.clone());
    extensions
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use notify::event::{DataChange, ModifyKind, RenameMode};
    use notify::EventKind;

    use super::{configured_local_source_roots, event_may_affect_index};
    use crate::config::Settings;

    #[test]
    fn configured_roots_include_only_existing_local_sources() {
        let root = tempfile_dir();
        let local = root.join("local");
        let missing = root.join("missing");
        fs::create_dir_all(&local).unwrap();
        let settings = Settings {
            image_sources: vec![
                local.to_string_lossy().to_string(),
                missing.to_string_lossy().to_string(),
                "s3://bucket/photos".to_string(),
            ],
            ..Settings::default()
        };

        let roots = configured_local_source_roots(&settings);

        assert_eq!(roots, [local.canonicalize().unwrap()].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watcher_events_filter_to_indexable_changes() {
        let settings = Settings::default();
        let image_write = notify::Event {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            paths: vec![PathBuf::from("/images/cat.JPG")],
            attrs: Default::default(),
        };
        let text_write = notify::Event {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            paths: vec![PathBuf::from("/images/readme.txt")],
            attrs: Default::default(),
        };
        let directory_rename = notify::Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            paths: vec![PathBuf::from("/images/album")],
            attrs: Default::default(),
        };

        assert!(event_may_affect_index(&image_write, &settings));
        assert!(!event_may_affect_index(&text_write, &settings));
        assert!(event_may_affect_index(&directory_rename, &settings));
    }

    fn tempfile_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "image-sim-watcher-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}

use std::collections::BTreeMap as LedgerBTreeMap;
use std::fs as ledger_fs;
use std::io::Write as LedgerWrite;
use std::path::{Path as LedgerPath, PathBuf as LedgerPathBuf};

use chrono::{DateTime as LedgerDateTime, Utc as LedgerUtc};
use jobs_core::JobContext as LedgerJobContext;
use uuid::Uuid as LedgerUuid;

use crate::workers::sources::SourceImage as LedgerSourceImage;

const INDEXING_LEDGER_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct IndexingLedger {
    pub version: u32,
    pub active_run: Option<IndexLedgerRun>,
}

impl IndexingLedger {
    pub(crate) fn load(path: &LedgerPath) -> Self {
        match ledger_fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(mut ledger) => {
                    if ledger.version == 0 {
                        ledger.version = INDEXING_LEDGER_VERSION;
                    }
                    ledger
                }
                Err(error) => {
                    tracing::warn!(
                        path = %path.display(),
                        %error,
                        "indexing ledger is invalid; ignoring it until the next write"
                    );
                    Self::empty()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::empty(),
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "could not read indexing ledger; ignoring it until the next write"
                );
                Self::empty()
            }
        }
    }

    pub(crate) fn empty() -> Self {
        Self {
            version: INDEXING_LEDGER_VERSION,
            active_run: None,
        }
    }

    pub(crate) fn save(&self, path: &LedgerPath) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            ledger_fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Could not create indexing ledger directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let temp_path = path.with_extension(format!("{}.tmp", LedgerUuid::new_v4().simple()));
        let json = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        let mut temp = ledger_fs::File::create(&temp_path)
            .map_err(|error| format!("Could not create indexing ledger temp file: {error}"))?;
        temp.write_all(json.as_bytes())
            .map_err(|error| format!("Could not write indexing ledger temp file: {error}"))?;
        temp.flush()
            .map_err(|error| format!("Could not flush indexing ledger temp file: {error}"))?;
        ledger_fs::rename(&temp_path, path)
            .map_err(|error| format!("Could not replace indexing ledger file: {error}"))
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct IndexLedgerRun {
    pub run_id: String,
    pub status: IndexLedgerRunStatus,
    pub collection: String,
    pub indexing_profile: String,
    pub started_at: LedgerDateTime<LedgerUtc>,
    pub updated_at: LedgerDateTime<LedgerUtc>,
    pub totals: IndexLedgerRunTotals,
    pub sources: LedgerBTreeMap<String, IndexLedgerSource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IndexLedgerRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct IndexLedgerRunTotals {
    pub pending: usize,
    pub already_indexed: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub pruned: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct IndexLedgerSource {
    pub source_uri: String,
    pub source_item_uri: String,
    pub display_path: String,
    pub size_bytes: u64,
    pub modified_at: f64,
    pub indexing_profile: String,
    pub status: IndexLedgerSourceStatus,
    pub committed_point_ids: Vec<String>,
    pub current_part: Option<IndexLedgerSourcePart>,
    pub error: Option<String>,
    pub updated_at: LedgerDateTime<LedgerUtc>,
}

impl IndexLedgerSource {
    pub(crate) fn matches_source(
        &self,
        source: &LedgerSourceImage,
        indexing_profile: &str,
    ) -> bool {
        self.source_item_uri == source.item_uri
            && self.size_bytes == source.size_bytes
            && (self.modified_at - source.modified_at).abs() <= 0.001
            && self.indexing_profile == indexing_profile
    }

    pub(crate) fn is_incomplete(&self) -> bool {
        matches!(
            self.status,
            IndexLedgerSourceStatus::Running
                | IndexLedgerSourceStatus::Failed
                | IndexLedgerSourceStatus::Cancelled
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IndexLedgerSourceStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct IndexLedgerSourcePart {
    pub kind: String,
    pub index: usize,
    pub total: usize,
}

pub(crate) struct IndexRunRecorder {
    path: LedgerPathBuf,
    ledger: IndexingLedger,
    context: Option<LedgerJobContext>,
    current_source_item_uri: Option<String>,
    current_source_progress: Option<IndexLedgerSourceProgress>,
}

#[derive(Clone, Debug)]
struct IndexLedgerSourceProgress {
    completed_before_source: u64,
    total_sources: u64,
    display_path: String,
}

impl IndexRunRecorder {
    pub(crate) fn start(
        path: LedgerPathBuf,
        run_id: String,
        collection: String,
        indexing_profile: String,
        totals: IndexLedgerRunTotals,
        context: Option<&LedgerJobContext>,
    ) -> Self {
        let now = LedgerUtc::now();
        let recorder = Self {
            path,
            ledger: IndexingLedger {
                version: INDEXING_LEDGER_VERSION,
                active_run: Some(IndexLedgerRun {
                    run_id,
                    status: IndexLedgerRunStatus::Running,
                    collection,
                    indexing_profile,
                    started_at: now,
                    updated_at: now,
                    totals,
                    sources: LedgerBTreeMap::new(),
                }),
            },
            context: context.cloned(),
            current_source_item_uri: None,
            current_source_progress: None,
        };
        recorder.metadata("ledger_path", recorder.path.to_string_lossy().to_string());
        recorder
    }

    pub(crate) fn register_pending_sources<'a>(
        &mut self,
        sources: impl IntoIterator<Item = &'a LedgerSourceImage>,
        indexing_profile: &str,
    ) {
        let now = LedgerUtc::now();
        if let Some(run) = self.ledger.active_run.as_mut() {
            run.updated_at = now;
            for source in sources {
                run.sources
                    .entry(source.item_uri.clone())
                    .or_insert_with(|| IndexLedgerSource {
                        source_uri: source.source_uri.clone(),
                        source_item_uri: source.item_uri.clone(),
                        display_path: source.display_path.clone(),
                        size_bytes: source.size_bytes,
                        modified_at: source.modified_at,
                        indexing_profile: indexing_profile.to_string(),
                        status: IndexLedgerSourceStatus::Running,
                        committed_point_ids: Vec::new(),
                        current_part: None,
                        error: None,
                        updated_at: now,
                    });
            }
        }
        self.persist();
    }

    pub(crate) fn source_started(
        &mut self,
        source: &LedgerSourceImage,
        indexing_profile: &str,
        completed_before_source: u64,
        total_sources: u64,
    ) {
        let now = LedgerUtc::now();
        self.current_source_item_uri = Some(source.item_uri.clone());
        self.current_source_progress = Some(IndexLedgerSourceProgress {
            completed_before_source,
            total_sources,
            display_path: source.display_path.clone(),
        });
        if let Some(run) = self.ledger.active_run.as_mut() {
            run.updated_at = now;
            run.sources.insert(
                source.item_uri.clone(),
                IndexLedgerSource {
                    source_uri: source.source_uri.clone(),
                    source_item_uri: source.item_uri.clone(),
                    display_path: source.display_path.clone(),
                    size_bytes: source.size_bytes,
                    modified_at: source.modified_at,
                    indexing_profile: indexing_profile.to_string(),
                    status: IndexLedgerSourceStatus::Running,
                    committed_point_ids: Vec::new(),
                    current_part: None,
                    error: None,
                    updated_at: now,
                },
            );
        }
        self.metadata("current_source", source.display_path.clone());
        self.metadata("current_source_item_uri", source.item_uri.clone());
        self.metadata("committed_points", "0");
        self.report_source_progress(None);
        self.persist();
    }

    pub(crate) fn current_part(&mut self, kind: &str, index: usize, total: usize) {
        self.with_current_source(|source| {
            source.current_part = Some(IndexLedgerSourcePart {
                kind: kind.to_string(),
                index,
                total,
            });
            source.updated_at = LedgerUtc::now();
        });
        self.metadata("current_part", format!("{kind}:{index}/{total}"));
        self.report_source_progress(Some(&IndexLedgerSourcePart {
            kind: kind.to_string(),
            index,
            total,
        }));
        self.persist();
    }

    pub(crate) fn committed_point(&mut self, point_id: &str) {
        let committed = self.with_current_source(|source| {
            if !source
                .committed_point_ids
                .iter()
                .any(|existing| existing == point_id)
            {
                source.committed_point_ids.push(point_id.to_string());
            }
            source.updated_at = LedgerUtc::now();
            source.committed_point_ids.len()
        });
        if let Some(committed) = committed {
            self.metadata("committed_points", committed.to_string());
        }
        self.persist();
    }

    pub(crate) fn source_completed(&mut self, point_ids: &[String]) {
        let committed = self.with_current_source(|source| {
            source.status = IndexLedgerSourceStatus::Completed;
            source.current_part = None;
            source.error = None;
            source.committed_point_ids = point_ids.to_vec();
            source.updated_at = LedgerUtc::now();
            source.committed_point_ids.len()
        });
        if let Some(committed) = committed {
            self.metadata("committed_points", committed.to_string());
        }
        self.current_source_item_uri = None;
        self.current_source_progress = None;
        self.persist();
    }

    pub(crate) fn source_failed(&mut self, error: &str) {
        self.mark_source(IndexLedgerSourceStatus::Failed, Some(error.to_string()));
    }

    pub(crate) fn source_cancelled(&mut self) {
        self.mark_source(
            IndexLedgerSourceStatus::Cancelled,
            Some("cancelled".to_string()),
        );
    }

    pub(crate) fn update_totals(
        &mut self,
        indexed: usize,
        skipped: usize,
        failed: usize,
        pruned: usize,
    ) {
        if let Some(run) = self.ledger.active_run.as_mut() {
            run.updated_at = LedgerUtc::now();
            run.totals.indexed = indexed;
            run.totals.skipped = skipped;
            run.totals.failed = failed;
            run.totals.pruned = pruned;
        }
        self.persist();
    }

    pub(crate) fn finish(&mut self, status: IndexLedgerRunStatus) {
        if let Some(run) = self.ledger.active_run.as_mut() {
            run.status = status;
            run.updated_at = LedgerUtc::now();
            let interrupted = run
                .sources
                .values()
                .filter(|source| source.is_incomplete())
                .count();
            self.metadata("interrupted_source_count", interrupted.to_string());
        }
        self.persist();
    }

    pub(crate) fn check_cancelled(&mut self) -> Result<(), String> {
        if let Some(context) = &self.context {
            if context.is_cancelled() {
                self.source_cancelled();
                return Err("job cancelled".to_string());
            }
        }
        Ok(())
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.context
            .as_ref()
            .map(|context| context.is_cancelled())
            .unwrap_or(false)
    }

    fn mark_source(&mut self, status: IndexLedgerSourceStatus, error: Option<String>) {
        self.with_current_source(|source| {
            source.status = status;
            source.error = error;
            source.updated_at = LedgerUtc::now();
        });
        self.persist();
    }

    fn with_current_source<T>(
        &mut self,
        update: impl FnOnce(&mut IndexLedgerSource) -> T,
    ) -> Option<T> {
        let key = self.current_source_item_uri.as_ref()?;
        let run = self.ledger.active_run.as_mut()?;
        run.updated_at = LedgerUtc::now();
        run.sources.get_mut(key).map(update)
    }

    fn persist(&self) {
        if let Err(error) = self.ledger.save(&self.path) {
            if let Some(context) = &self.context {
                let _ = context.warn(format!("could not save indexing ledger: {error}"));
            } else {
                tracing::warn!(%error, "could not save indexing ledger");
            }
        }
    }

    fn metadata(&self, key: &str, value: impl Into<String>) {
        if let Some(context) = &self.context {
            let _ = context.metadata(key, value.into());
        }
    }

    fn report_source_progress(&self, part: Option<&IndexLedgerSourcePart>) {
        let Some(context) = &self.context else {
            return;
        };
        let Some(progress) = &self.current_source_progress else {
            return;
        };

        let source_number = progress.completed_before_source + 1;
        let mut message = format!(
            "indexing source {source_number}/{}: {}",
            progress.total_sources, progress.display_path
        );
        if let Some(part) = part {
            message.push_str(&format!(
                " ({} {}/{})",
                part.kind,
                part.index + 1,
                part.total
            ));
        }

        let part_total = part.map(|part| part.total as u64).unwrap_or(2).max(1);
        let part_completed = part
            .map(|part| part.index as u64 + 1)
            .unwrap_or(1)
            .min(part_total);
        let total_steps = progress.total_sources.saturating_mul(part_total).max(1);
        let completed_steps = progress
            .completed_before_source
            .saturating_mul(part_total)
            .saturating_add(part_completed)
            .min(total_steps);
        let job_progress = jobs_core::JobProgress::new(completed_steps, Some(total_steps))
            .and_then(|progress| progress.unit("steps"))
            .map(|progress| progress.message(message))
            .and_then(|progress| {
                progress.validate()?;
                Ok(progress)
            });
        if let Ok(progress) = job_progress {
            let _ = context.progress(progress);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{
        IndexLedgerRun, IndexLedgerRunStatus, IndexLedgerRunTotals, IndexLedgerSource,
        IndexLedgerSourceStatus, IndexRunRecorder, IndexingLedger,
    };
    use crate::workers::sources::SourceImage;

    #[test]
    fn missing_ledger_loads_as_empty() {
        let path = temp_path("missing.json");
        let ledger = IndexingLedger::load(&path);

        assert!(ledger.active_run.is_none());
        assert_eq!(ledger.version, 1);
    }

    #[test]
    fn invalid_ledger_is_ignored_and_replaced_on_save() {
        let path = temp_path("invalid.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not json").unwrap();

        let mut ledger = IndexingLedger::load(&path);
        ledger.active_run = Some(run());
        ledger.save(&path).unwrap();

        let parsed = IndexingLedger::load(&path);
        assert!(parsed.active_run.is_some());
    }

    #[test]
    fn save_creates_parent_and_writes_parseable_json() {
        let path = temp_path("nested/ledger.json");
        let ledger = IndexingLedger {
            version: 1,
            active_run: Some(run()),
        };

        ledger.save(&path).unwrap();

        let parsed = IndexingLedger::load(&path);
        assert_eq!(
            parsed.active_run.unwrap().status,
            IndexLedgerRunStatus::Running
        );
    }

    #[test]
    fn incomplete_statuses_are_reported() {
        for status in [
            IndexLedgerSourceStatus::Running,
            IndexLedgerSourceStatus::Failed,
            IndexLedgerSourceStatus::Cancelled,
        ] {
            let source = source(status, Vec::new());
            assert!(source.is_incomplete());
        }

        assert!(!source(
            IndexLedgerSourceStatus::Completed,
            vec!["point".to_string()]
        )
        .is_incomplete());
    }

    #[test]
    fn source_signature_matches_current_file() {
        let image = SourceImage::test_local_image("/images/cat.jpg", 42, 100.0);
        let source = source(
            IndexLedgerSourceStatus::Completed,
            vec!["point".to_string()],
        );

        assert!(source.matches_source(&image, "profile"));
        assert!(!source.matches_source(&image, "other"));
    }

    #[test]
    fn recorder_keeps_committed_points_on_cancelled_source() {
        let path = temp_path("cancelled.json");
        let image = SourceImage::test_local_image("/images/cat.jpg", 42, 100.0);
        let mut recorder = IndexRunRecorder::start(
            path.clone(),
            "run".to_string(),
            "collection".to_string(),
            "profile".to_string(),
            IndexLedgerRunTotals {
                pending: 1,
                ..IndexLedgerRunTotals::default()
            },
            None,
        );

        recorder.source_started(&image, "profile", 0, 1);
        recorder.current_part("pdf_page", 0, 2);
        recorder.committed_point("page-1");
        recorder.source_cancelled();
        recorder.finish(IndexLedgerRunStatus::Cancelled);

        let ledger = IndexingLedger::load(&path);
        let mut run = ledger.active_run.unwrap();
        let source = run.sources.remove("/images/cat.jpg").unwrap();
        assert_eq!(source.status, IndexLedgerSourceStatus::Cancelled);
        assert_eq!(source.committed_point_ids, vec!["page-1"]);
        assert!(source.is_incomplete());
    }

    fn run() -> IndexLedgerRun {
        IndexLedgerRun {
            run_id: "run".to_string(),
            status: IndexLedgerRunStatus::Running,
            collection: "collection".to_string(),
            indexing_profile: "profile".to_string(),
            started_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            totals: IndexLedgerRunTotals::default(),
            sources: Default::default(),
        }
    }

    fn source(
        status: IndexLedgerSourceStatus,
        committed_point_ids: Vec<String>,
    ) -> IndexLedgerSource {
        IndexLedgerSource {
            source_uri: "/images".to_string(),
            source_item_uri: "/images/cat.jpg".to_string(),
            display_path: "/images/cat.jpg".to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: "profile".to_string(),
            status,
            committed_point_ids,
            current_part: None,
            error: None,
            updated_at: chrono::Utc::now(),
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("image-sim-ledger-test-{}", uuid::Uuid::new_v4()))
            .join(name)
    }
}

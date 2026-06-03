use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jobs_core::{
    BackgroundJobRunner, JobContext, JobError, JobEvent, JobId, JobJoinHandle, JobSnapshot, JobSpec,
};

#[derive(Clone, Default)]
pub struct JobManager {
    runner: BackgroundJobRunner,
    active: Arc<Mutex<BTreeMap<JobId, JobJoinHandle>>>,
}

impl JobManager {
    pub fn spawn<F>(&self, spec: JobSpec, run: F) -> jobs_core::Result<JobSnapshot>
    where
        F: FnOnce(JobContext) -> jobs_core::Result<()> + Send + 'static,
    {
        let handle = self.runner.spawn(spec, run)?;
        let id = handle.id().clone();
        let snapshot = self
            .runner
            .tracker()
            .snapshot(&id)?
            .ok_or_else(|| JobError::StateUnavailable(format!("job `{id}` was not tracked")))?;
        self.active_jobs()?.insert(id, handle);
        Ok(snapshot)
    }

    pub fn snapshots(&self) -> jobs_core::Result<Vec<JobSnapshot>> {
        self.runner.tracker().snapshots()
    }

    pub fn snapshot(&self, id: &JobId) -> jobs_core::Result<Option<JobSnapshot>> {
        self.runner.tracker().snapshot(id)
    }

    pub fn events(&self, id: &JobId) -> jobs_core::Result<Vec<JobEvent>> {
        self.runner.tracker().events(id)
    }

    pub fn has_active_kind_prefix(&self, prefix: &str) -> jobs_core::Result<bool> {
        Ok(self.snapshots()?.iter().any(|snapshot| {
            snapshot
                .spec
                .kind
                .as_deref()
                .map(|kind| kind.starts_with(prefix))
                .unwrap_or(false)
                && !snapshot.status.is_terminal()
        }))
    }

    pub fn request_cancel(&self, id: &JobId) -> jobs_core::Result<()> {
        let active = self.active_jobs()?;
        let handle = active
            .get(id)
            .ok_or_else(|| JobError::InvalidArgument(format!("unknown active job `{id}`")))?;
        handle.request_cancel()
    }

    pub fn request_cancel_kind_prefix(&self, prefix: &str) -> jobs_core::Result<Vec<JobId>> {
        let ids = self
            .snapshots()?
            .into_iter()
            .filter(|snapshot| {
                snapshot
                    .spec
                    .kind
                    .as_deref()
                    .map(|kind| kind.starts_with(prefix))
                    .unwrap_or(false)
                    && !snapshot.status.is_terminal()
            })
            .map(|snapshot| snapshot.spec.id)
            .collect::<Vec<_>>();
        let active = self.active_jobs()?;
        for id in &ids {
            if let Some(handle) = active.get(id) {
                handle.request_cancel()?;
            }
        }
        Ok(ids)
    }

    pub fn wait_for_terminal(&self, ids: &[JobId], timeout: Duration) -> jobs_core::Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let snapshots = self.snapshots()?;
            let running = ids
                .iter()
                .filter(|id| {
                    snapshots
                        .iter()
                        .find(|snapshot| snapshot.spec.id == **id)
                        .map(|snapshot| !snapshot.status.is_terminal())
                        .unwrap_or(false)
                })
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            if running.is_empty() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(JobError::Failed(format!(
                    "timed out waiting for job(s) to stop: {}",
                    running.join(", ")
                )));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn active_jobs(
        &self,
    ) -> jobs_core::Result<std::sync::MutexGuard<'_, BTreeMap<JobId, JobJoinHandle>>> {
        self.active
            .lock()
            .map_err(|_| JobError::StateUnavailable("active job registry lock poisoned".into()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jobs_core::{JobSpec, JobStatus};
    use uuid::Uuid;

    use super::JobManager;

    #[test]
    fn cancels_and_waits_for_jobs_by_kind_prefix() {
        let jobs = JobManager::default();
        let spec = JobSpec::new(
            format!("index.manual.{}", Uuid::new_v4()),
            "Cancellable index job",
        )
        .and_then(|spec| spec.with_kind("index.manual"))
        .unwrap();
        let snapshot = jobs
            .spawn(spec, |context| loop {
                context.check_cancelled()?;
                std::thread::sleep(Duration::from_millis(5));
            })
            .unwrap();

        let cancelled = jobs.request_cancel_kind_prefix("index.").unwrap();
        assert_eq!(cancelled, vec![snapshot.spec.id.clone()]);
        jobs.wait_for_terminal(&cancelled, Duration::from_secs(2))
            .unwrap();

        let snapshot = jobs.snapshot(&snapshot.spec.id).unwrap().unwrap();
        assert_eq!(snapshot.status, JobStatus::Cancelled);
    }
}

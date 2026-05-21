use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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

    pub fn request_cancel(&self, id: &JobId) -> jobs_core::Result<()> {
        let active = self.active_jobs()?;
        let handle = active
            .get(id)
            .ok_or_else(|| JobError::InvalidArgument(format!("unknown active job `{id}`")))?;
        handle.request_cancel()
    }

    fn active_jobs(
        &self,
    ) -> jobs_core::Result<std::sync::MutexGuard<'_, BTreeMap<JobId, JobJoinHandle>>> {
        self.active
            .lock()
            .map_err(|_| JobError::StateUnavailable("active job registry lock poisoned".into()))
    }
}

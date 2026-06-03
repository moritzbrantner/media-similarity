import { Button } from "@moritzbrantner/ui";
import { Card, CardContent, CardHeader, CardTitle } from "@moritzbrantner/ui";
import { Progress } from "@moritzbrantner/ui";
import { AlertCircle, Database, Loader2, X } from "lucide-react";
import {
  formatHistoryTime,
  formatJobTime,
  jobEventText,
  jobIsActive,
  jobStatusClass,
} from "../jobs/job-utils";
import type { JobEvent, JobSnapshot } from "../types";
import { Message } from "./status-message";

export function JobsPanel({
  cancelPendingJobId,
  error,
  events,
  jobs,
  onCancel,
  onSelectJob,
  selectedJobId,
}: {
  cancelPendingJobId: string | null;
  error: Error | null;
  events: JobEvent[];
  jobs: JobSnapshot[];
  onCancel: (jobId: string) => void;
  onSelectJob: (jobId: string) => void;
  selectedJobId: string | null;
}) {
  const selectedJob = jobs.find((job) => job.spec.id === selectedJobId) ?? jobs[0] ?? null;
  const selectedJobIsCancelling = selectedJob?.status === "Cancelling";
  const selectedJobCancelPending =
    selectedJob !== null && (cancelPendingJobId === selectedJob.spec.id || selectedJobIsCancelling);
  const recentEvents = events.slice(-5).reverse();

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  return (
    <Card className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <CardHeader className="flex flex-col gap-3 p-0 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Database className="size-4 text-neutral-600" aria-hidden="true" />
            <CardTitle
              aria-level={2}
              className="text-sm font-semibold text-neutral-950"
              role="heading"
            >
              Background Jobs
            </CardTitle>
          </div>
          <p className="mt-1 text-sm text-neutral-600">
            {selectedJob
              ? `${selectedJob.spec.name} · ${selectedJob.status}`
              : "No background jobs yet."}
          </p>
        </div>
        {selectedJob && jobIsActive(selectedJob) ? (
          <Button
            variant="outline"
            className="inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md border border-red-200 bg-white px-3 text-sm font-semibold text-red-700 transition hover:bg-red-50 disabled:cursor-wait disabled:opacity-60"
            disabled={selectedJobCancelPending}
            onClick={() => onCancel(selectedJob.spec.id)}
            type="button"
          >
            {selectedJobCancelPending ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <X className="size-4" aria-hidden="true" />
            )}
            <span>{selectedJobIsCancelling ? "Cancelling" : "Cancel"}</span>
          </Button>
        ) : null}
      </CardHeader>

      {selectedJob ? (
        <CardContent className="mt-4 grid gap-4 p-0 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.9fr)]">
          <div className="min-w-0">
            <div className="flex items-center justify-between gap-3 text-sm">
              <span className={`font-semibold ${jobStatusClass(selectedJob.status)}`}>
                {selectedJob.status}
              </span>
              <span className="text-neutral-600">{formatJobTime(selectedJob)}</span>
            </div>
            <JobProgressBar progress={selectedJob.progress} />
            <div className="mt-3 flex flex-wrap gap-2">
              {jobs.slice(0, 6).map((job) => (
                <Button
                  variant={job.spec.id === selectedJob.spec.id ? "default" : "outline"}
                  className={`max-w-full rounded-md border px-2 py-1 text-left text-xs transition ${
                    job.spec.id === selectedJob.spec.id
                      ? "border-neutral-900 bg-neutral-900 text-white"
                      : "border-neutral-300 bg-neutral-50 text-neutral-700 hover:border-neutral-500"
                  }`}
                  key={job.spec.id}
                  onClick={() => onSelectJob(job.spec.id)}
                  title={job.spec.id}
                  type="button"
                >
                  <span className="block max-w-44 truncate font-semibold">{job.spec.name}</span>
                  <span className="block">{job.status}</span>
                </Button>
              ))}
            </div>
          </div>

          <div className="min-w-0">
            <h3 className="text-xs font-semibold uppercase tracking-normal text-neutral-500">
              Recent Events
            </h3>
            <ol className="mt-2 grid max-h-40 gap-2 overflow-auto pr-1">
              {recentEvents.length > 0 ? (
                recentEvents.map((event) => (
                  <li className="rounded-md bg-neutral-50 px-3 py-2 text-sm" key={event.sequence}>
                    <div className="flex items-start justify-between gap-3">
                      <span className="min-w-0 text-neutral-800">{jobEventText(event)}</span>
                      <span className="shrink-0 text-xs text-neutral-500">
                        {formatHistoryTime(event.timestamp)}
                      </span>
                    </div>
                  </li>
                ))
              ) : (
                <li className="rounded-md bg-neutral-50 px-3 py-2 text-sm text-neutral-500">
                  No events recorded.
                </li>
              )}
            </ol>
          </div>
        </CardContent>
      ) : null}
    </Card>
  );
}

function JobProgressBar({ progress }: { progress: JobSnapshot["progress"] }) {
  const percent =
    progress?.total && progress.total > 0
      ? Math.min(100, Math.round((progress.completed / progress.total) * 100))
      : null;
  const value = progress
    ? `${progress.completed}${progress.total ? `/${progress.total}` : ""} ${progress.unit}`
    : "Waiting";

  return (
    <div className="mt-2">
      <div className="flex items-center justify-between gap-3 text-xs text-neutral-600">
        <span className="min-w-0 truncate">{progress?.message ?? value}</span>
        <span className="shrink-0">{percent === null ? value : `${percent}%`}</span>
      </div>
      <Progress className="mt-2 h-2 bg-neutral-200" value={percent ?? (progress ? 45 : 0)} />
    </div>
  );
}

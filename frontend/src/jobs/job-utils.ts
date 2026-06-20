import type { JobEvent, JobSnapshot } from "../types";

type JobStatus = JobSnapshot["status"];

export function formatHistoryTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

export function sortJobs(jobs: JobSnapshot[]) {
  return [...jobs].sort(
    (left, right) => new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
  );
}

export function jobIsActive(jobOrStatus: JobSnapshot | JobStatus) {
  const status = typeof jobOrStatus === "string" ? jobOrStatus : jobOrStatus.status;
  return status === "Queued" || status === "Running" || status === "Cancelling";
}

export function jobIsTerminal(status: JobSnapshot["status"]) {
  return status === "Succeeded" || status === "Failed" || status === "Cancelled";
}

export function numberFromMetadata(value: string | undefined) {
  if (value === undefined) {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function jobStatusClass(status: JobSnapshot["status"]) {
  return {
    Cancelled: "text-amber-700",
    Cancelling: "text-amber-700",
    Failed: "text-red-700",
    Queued: "text-neutral-700",
    Running: "text-emerald-700",
    Succeeded: "text-emerald-700",
  }[status];
}

export function formatJobTime(job: JobSnapshot) {
  const value = job.finished_at ?? job.started_at ?? job.created_at;
  return formatHistoryTime(value);
}

export function jobEventText(event: JobEvent) {
  const kind = event.kind;
  if ("StatusChanged" in kind) {
    return kind.StatusChanged.message
      ? `${kind.StatusChanged.status}: ${kind.StatusChanged.message}`
      : kind.StatusChanged.status;
  }
  if ("Progress" in kind) {
    const progress = kind.Progress;
    const total = progress.total ? `/${progress.total}` : "";
    return progress.message ?? `${progress.completed}${total} ${progress.unit}`;
  }
  if ("Log" in kind) {
    return kind.Log.message;
  }
  if ("Metadata" in kind) {
    return `${kind.Metadata.key}: ${kind.Metadata.value}`;
  }
  return "Artifact recorded";
}

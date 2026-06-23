export type JobStatus = "Queued" | "Running" | "Cancelling" | "Succeeded" | "Failed" | "Cancelled";

export type JobProgress = {
  completed: number;
  total: number | null;
  unit: string;
  message: string | null;
};

export type JobLogEntry = {
  timestamp: string;
  level: "Debug" | "Info" | "Warn" | "Error";
  message: string;
};

export type JobSpec = {
  id: string;
  name: string;
  kind: string | null;
  metadata: Record<string, string>;
};

export type JobSnapshot = {
  spec: JobSpec;
  status: JobStatus;
  progress: JobProgress | null;
  logs: JobLogEntry[];
  artifacts: unknown[];
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  failure: { message: string } | null;
  metadata: Record<string, string>;
};

export type JobEvent = {
  job_id: string;
  sequence: number;
  timestamp: string;
  kind:
    | { StatusChanged: { status: JobStatus; message: string | null } }
    | { Progress: JobProgress }
    | { Log: JobLogEntry }
    | { Artifact: unknown }
    | { Metadata: { key: string; value: string } };
};

import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { cancelJob, fetchJobEvents, fetchJobs, startIndexJob } from "../../api";
import { jobIsActive, jobIsTerminal, numberFromMetadata, sortJobs } from "../../jobs/job-utils";
import type { HealthResponse, IndexResponse } from "../../types";

export function useJobsController({ healthData }: { healthData?: HealthResponse }) {
  const queryClient = useQueryClient();
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const refreshedModelJobIdRef = useRef<string | null>(null);

  const jobsQuery = useQuery({
    queryKey: ["jobs"],
    queryFn: fetchJobs,
    refetchInterval: 2000,
  });

  const jobs = useMemo(() => sortJobs(jobsQuery.data ?? []), [jobsQuery.data]);
  const latestIndexJob = jobs.find((job) => job.spec.kind?.startsWith("index."));
  const latestModelJob = jobs.find((job) => job.spec.kind?.startsWith("model."));
  const selectedJob = jobs.find((job) => job.spec.id === selectedJobId) ?? jobs[0] ?? null;

  const indexMutation = useMutation({
    mutationFn: startIndexJob,
    onSuccess: (job) => {
      setSelectedJobId(job.spec.id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
  });

  const cancelJobMutation = useMutation({
    mutationFn: cancelJob,
    onSuccess: (job) => {
      setSelectedJobId(job.spec.id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["job-events", job.spec.id] });
    },
  });

  const jobEventsQuery = useQuery({
    queryKey: ["job-events", selectedJob?.spec.id],
    queryFn: () => fetchJobEvents(selectedJob?.spec.id ?? ""),
    enabled: Boolean(selectedJob),
    refetchInterval: selectedJob && jobIsActive(selectedJob.status) ? 1500 : false,
  });

  const lastIndex = useMemo<IndexResponse | null>(() => {
    if (!latestIndexJob || !jobIsTerminal(latestIndexJob.status)) {
      return null;
    }

    const indexed = numberFromMetadata(latestIndexJob.metadata.indexed);
    const alreadyIndexed = numberFromMetadata(latestIndexJob.metadata.already_indexed);
    const skipped = numberFromMetadata(latestIndexJob.metadata.skipped);
    const failed = numberFromMetadata(latestIndexJob.metadata.failed);
    if (indexed === null || skipped === null || failed === null) {
      return null;
    }

    return {
      collection: latestIndexJob.metadata.collection ?? healthData?.collection ?? "",
      errors: latestIndexJob.logs
        .filter((entry) => entry.level === "Warn" || entry.level === "Error")
        .map((entry) => entry.message),
      failed,
      indexed,
      already_indexed: alreadyIndexed ?? 0,
      pruned: numberFromMetadata(latestIndexJob.metadata.pruned) ?? 0,
      skipped,
      source_dir: healthData?.source_dir ?? "",
      sources: healthData?.sources ?? [],
    };
  }, [healthData, latestIndexJob]);

  useEffect(() => {
    if (latestIndexJob?.status !== "Succeeded") {
      return;
    }
    queryClient.invalidateQueries({ queryKey: ["health"] });
    queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
  }, [latestIndexJob?.spec.id, latestIndexJob?.status, queryClient]);

  useEffect(() => {
    if (
      !latestModelJob ||
      !jobIsTerminal(latestModelJob.status) ||
      latestModelJob.spec.id === refreshedModelJobIdRef.current
    ) {
      return;
    }

    refreshedModelJobIdRef.current = latestModelJob.spec.id;
    queryClient.invalidateQueries({ queryKey: ["models"] });
    queryClient.invalidateQueries({ queryKey: ["health"] });
    queryClient.invalidateQueries({ queryKey: ["source-config"] });
  }, [latestModelJob, queryClient]);

  const indexActive = Boolean(latestIndexJob && jobIsActive(latestIndexJob.status));

  return {
    cancelJobMutation,
    jobs,
    jobsQuery,
    jobEventsQuery,
    indexActive,
    indexMutation,
    latestIndexJob,
    latestModelJob,
    lastIndex,
    selectedJob,
    selectedJobId: selectedJob?.spec.id ?? null,
    setSelectedJobId,
    indexError: indexMutation.error,
  };
}

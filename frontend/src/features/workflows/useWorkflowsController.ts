import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { fetchWorkflows, resetWorkflows, updateWorkflows, validateWorkflows } from "../../api";
import type { WorkflowEditorLibrary, MediaWorkflowNodeData } from "../../types";

export function useWorkflowsController({ workflowsEnabled }: { workflowsEnabled: boolean }) {
  const queryClient = useQueryClient();

  const workflowsQuery = useQuery({
    queryKey: ["workflows"],
    queryFn: fetchWorkflows,
    enabled: workflowsEnabled,
  });

  const workflowMutation = useMutation({
    mutationFn: updateWorkflows,
    onSuccess: (response) => {
      queryClient.setQueryData(["workflows"], response);
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const workflowResetMutation = useMutation({
    mutationFn: resetWorkflows,
    onSuccess: (response) => {
      queryClient.setQueryData(["workflows"], response);
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const workflowValidateMutation = useMutation({
    mutationFn: (library: WorkflowEditorLibrary<MediaWorkflowNodeData>) =>
      validateWorkflows(library),
  });

  return {
    workflowMutation,
    workflowResetMutation,
    workflowValidateMutation,
    workflowsQuery,
  };
}

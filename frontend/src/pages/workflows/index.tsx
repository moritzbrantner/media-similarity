import { useWorkflowsController } from "../../features/workflows/useWorkflowsController";
import { useAppShellContext } from "../_layout";
import { WorkflowConfigurationPage } from "./workflow-configuration-page";

export function WorkflowsRoute() {
  const shell = useAppShellContext();
  const workflowController = useWorkflowsController({
    workflowsEnabled: true,
  });

  return (
    <WorkflowConfigurationPage
      config={workflowController.workflowsQuery.data ?? null}
      error={workflowController.workflowsQuery.error}
      indexError={shell.indexError}
      indexPending={shell.indexPending || shell.indexActive}
      lastIndex={shell.lastIndex}
      loading={workflowController.workflowsQuery.isLoading}
      onIndex={shell.onIndex}
      onReset={() => workflowController.workflowResetMutation.mutate()}
      onSave={(library) => workflowController.workflowMutation.mutate(library)}
      onValidate={(library) =>
        workflowController.workflowValidateMutation
          .mutateAsync(library)
          .then((response) => response.diagnostics)
      }
      resetPending={workflowController.workflowResetMutation.isPending}
      saveError={workflowController.workflowMutation.error}
      savePending={workflowController.workflowMutation.isPending}
      saveSuccess={workflowController.workflowMutation.isSuccess}
      validateError={workflowController.workflowValidateMutation.error}
      validatePending={workflowController.workflowValidateMutation.isPending}
    />
  );
}

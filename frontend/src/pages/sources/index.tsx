import { useConfigurationController } from "../../features/configuration/useConfigurationController";
import { useAppShellContext } from "../_layout";
import { SourceConfigurationPage } from "./source-configuration-page";

export function SourcesRoute() {
  const shell = useAppShellContext();
  const configController = useConfigurationController({
    sourceConfigEnabled: true,
  });

  return (
    <SourceConfigurationPage
      config={configController.sourceConfigQuery.data ?? null}
      error={configController.sourceConfigQuery.error}
      indexError={shell.indexError}
      indexPending={shell.indexPending || shell.indexActive}
      lastIndex={shell.lastIndex}
      loading={configController.sourceConfigQuery.isLoading}
      modelActionPending={configController.modelActionPending}
      modelError={configController.modelError}
      models={configController.modelsQuery.data ?? null}
      modelsError={configController.modelsQuery.error}
      modelsLoading={configController.modelsQuery.isLoading}
      onDownloadAllModels={() => configController.downloadAllModelsMutation.mutate()}
      onDownloadModel={(role, model) =>
        configController.downloadModelMutation.mutate({ role, model })
      }
      onDisableModel={(role) => configController.disableModelMutation.mutate({ role })}
      onEnableModel={(role, model) => configController.enableModelMutation.mutate({ role, model })}
      onIndex={shell.onIndex}
      onPreview={(sources) => configController.sourcePreviewMutation.mutate(sources)}
      onSave={(sources) => configController.sourceConfigMutation.mutate(sources)}
      preview={configController.sourcePreviewMutation.data ?? null}
      previewError={configController.sourcePreviewMutation.error}
      previewPending={configController.sourcePreviewMutation.isPending}
      saveError={configController.sourceConfigMutation.error}
      savePending={configController.sourceConfigMutation.isPending}
      saveSuccess={configController.sourceConfigMutation.isSuccess}
    />
  );
}

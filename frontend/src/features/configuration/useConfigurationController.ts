import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  disableModel,
  downloadAllModels,
  downloadModel,
  enableModel,
  fetchModels,
  fetchSourceConfig,
  updateIndexingConfig,
  updateSourceConfig,
} from "../../api";

export function useConfigurationController({
  sourceConfigEnabled,
}: {
  sourceConfigEnabled: boolean;
}) {
  const queryClient = useQueryClient();

  const sourceConfigQuery = useQuery({
    queryKey: ["source-config"],
    queryFn: fetchSourceConfig,
    enabled: sourceConfigEnabled,
  });

  const modelsQuery = useQuery({
    queryKey: ["models"],
    queryFn: fetchModels,
    enabled: true,
  });

  const sourceConfigMutation = useMutation({
    mutationFn: updateSourceConfig,
    onSuccess: (response) => {
      queryClient.setQueryData(["source-config"], response);
      queryClient.invalidateQueries({ queryKey: ["health"] });
    },
  });

  const indexingConfigMutation = useMutation({
    mutationFn: updateIndexingConfig,
    onSuccess: (response) => {
      queryClient.setQueryData(["source-config"], response);
      queryClient.invalidateQueries({ queryKey: ["health"] });
    },
  });

  const downloadModelMutation = useMutation({
    mutationFn: ({ model, role }: { model?: string | null; role: string }) =>
      downloadModel(role, model),
    onSuccess: (_response) => {
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["models"] });
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const downloadAllModelsMutation = useMutation({
    mutationFn: downloadAllModels,
    onSuccess: (_response) => {
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["models"] });
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const enableModelMutation = useMutation({
    mutationFn: ({ role, model }: { role: string; model?: string | null }) =>
      enableModel(role, model),
    onSuccess: (_response) => {
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["models"] });
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const disableModelMutation = useMutation({
    mutationFn: ({ role }: { role: string }) => disableModel(role),
    onSuccess: (_response) => {
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["models"] });
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["source-config"] });
    },
  });

  const modelActionPending = useMemo(() => {
    if (downloadAllModelsMutation.isPending) {
      return "all";
    }
    if (
      downloadModelMutation.isPending ||
      enableModelMutation.isPending ||
      disableModelMutation.isPending
    ) {
      const active =
        downloadModelMutation.variables ??
        enableModelMutation.variables ??
        disableModelMutation.variables;
      return (active as { role: string })?.role;
    }
    return undefined;
  }, [
    downloadAllModelsMutation.isPending,
    downloadModelMutation.isPending,
    enableModelMutation.isPending,
    disableModelMutation.isPending,
    downloadModelMutation.variables,
    enableModelMutation.variables,
    disableModelMutation.variables,
  ]);

  const modelError =
    downloadAllModelsMutation.error ??
    downloadModelMutation.error ??
    enableModelMutation.error ??
    disableModelMutation.error;

  return {
    disableModelMutation,
    downloadAllModelsMutation,
    downloadModelMutation,
    enableModelMutation,
    indexingConfigMutation,
    modelsQuery,
    modelActionPending,
    sourceConfigMutation,
    sourceConfigQuery,
    modelError: modelError as Error | null,
  };
}

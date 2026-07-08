import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { fetchInverseIndex, mergeIdentities, renameIdentity } from "../../api";
import type { IdentityKind } from "../../api";
import { SEARCH_HISTORY_QUERY_KEY } from "../../search/defaults";
import { applyIdentityMutationToHistory } from "../../search/history";
import type { SearchHistoryItem } from "../../search/types";
import { InverseIndexPage } from "./inverse-index-page";

export function RegistryRoute() {
  const queryClient = useQueryClient();
  const inverseIndexQuery = useQuery({
    queryKey: ["inverse-index"],
    queryFn: fetchInverseIndex,
  });

  const sourceKindMutation = useMutation({
    mutationFn: ({ id, kind, label }: { id: string; kind: IdentityKind; label: string }) =>
      renameIdentity(kind, id, label),
    onSuccess: (response) => {
      queryClient.setQueryData<SearchHistoryItem[]>(SEARCH_HISTORY_QUERY_KEY, (history = []) =>
        applyIdentityMutationToHistory(history, response),
      );
      invalidateIdentityQueries(queryClient);
    },
  });

  const mergeIdentitiesMutation = useMutation({
    mutationFn: ({
      kind,
      sourceIds,
      targetId,
    }: {
      kind: IdentityKind;
      sourceIds: string[];
      targetId: string;
    }) => mergeIdentities(kind, targetId, sourceIds),
    onSuccess: (response) => {
      queryClient.setQueryData<SearchHistoryItem[]>(SEARCH_HISTORY_QUERY_KEY, (history = []) =>
        applyIdentityMutationToHistory(history, response),
      );
      invalidateIdentityQueries(queryClient);
    },
  });

  return (
    <InverseIndexPage
      data={inverseIndexQuery.data ?? null}
      error={inverseIndexQuery.error}
      loading={inverseIndexQuery.isLoading}
      mergeError={mergeIdentitiesMutation.error}
      mergeErrorIdentity={
        mergeIdentitiesMutation.isError && mergeIdentitiesMutation.variables
          ? {
              id: mergeIdentitiesMutation.variables.targetId,
              kind: mergeIdentitiesMutation.variables.kind,
            }
          : null
      }
      mergingIdentity={
        mergeIdentitiesMutation.isPending && mergeIdentitiesMutation.variables
          ? {
              id: mergeIdentitiesMutation.variables.targetId,
              kind: mergeIdentitiesMutation.variables.kind,
            }
          : null
      }
      onMergeIdentity={(kind, targetId, sourceIds) =>
        mergeIdentitiesMutation.mutateAsync({
          kind,
          sourceIds,
          targetId,
        })
      }
      onRefresh={() => {
        inverseIndexQuery.refetch();
      }}
      onRenameIdentity={(kind, id, label) => sourceKindMutation.mutateAsync({ id, kind, label })}
      refreshing={inverseIndexQuery.isFetching}
      renameError={sourceKindMutation.error}
      renameErrorIdentity={
        sourceKindMutation.isError && sourceKindMutation.variables
          ? {
              id: sourceKindMutation.variables.id,
              kind: sourceKindMutation.variables.kind,
            }
          : null
      }
      renamingIdentity={
        sourceKindMutation.isPending && sourceKindMutation.variables
          ? {
              id: sourceKindMutation.variables.id,
              kind: sourceKindMutation.variables.kind,
            }
          : null
      }
    />
  );
}

function invalidateIdentityQueries(queryClient: ReturnType<typeof useQueryClient>) {
  queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
}

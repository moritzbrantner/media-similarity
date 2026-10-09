import type { FormEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  SearchHistoryItem,
  SearchVariables,
  MetadataFilters,
  ResultSortMode,
  SearchMode,
} from "../../search/types";
import { createQueryPreview } from "../../search/preview";
import {
  applyIdentityMutationToHistory,
  removeResultFromFaceResponse,
  removeResultFromResponse,
  updateMediaInFaceResponse,
  updateMediaInResponse,
  loadSearchHistory,
  saveSearchHistory,
} from "../../search/history";
import { filterResults, sourceTypesFor } from "../../search/filtering";
import {
  DEFAULT_LIMIT,
  DEFAULT_METADATA_FILTERS,
  DEFAULT_RESULT_SORT,
  MAX_SEARCH_HISTORY,
  SEARCH_HISTORY_QUERY_KEY,
} from "../../search/defaults";
import { sortResults } from "../../search/sorting";
import {
  searchMedia,
  searchFaceMedia,
  deleteIndexedMedia,
  updateIndexedMediaTags,
} from "../../api";
import { isAudioFile, isPdfFile } from "../../lib/media";
import type { FaceSearchResponse, IdentityMutationResponse, SearchResult } from "../../types";

type FaceSearchVariables = {
  filters: MetadataFilters;
  queryFile: File;
  resultLimit: number;
};

export function useSearchController() {
  const queryClient = useQueryClient();

  const [file, setFile] = useState<File | null>(null);
  const [limit, setLimit] = useState(DEFAULT_LIMIT);
  const [metadataFilters, setMetadataFilters] = useState<MetadataFilters>(DEFAULT_METADATA_FILTERS);
  const [ocrTextQuery, setOcrTextQuery] = useState("");
  const activeFaceQuery = useRef<FaceSearchVariables | null>(null);
  const currentFaceSnapshot = useRef<{
    source: FaceSearchResponse;
    response: FaceSearchResponse;
  } | null>(null);
  const faceRefreshPending = useRef(false);
  const faceQueryGeneration = useRef(0);
  const [searchMode, setSearchMode] = useState<SearchMode>("media");
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [resultSortMode, setResultSortMode] = useState<ResultSortMode>(DEFAULT_RESULT_SORT);
  const [activeSearchId, setActiveSearchId] = useState<string | null>(null);
  const [selectedQuerySceneIndex, setSelectedQuerySceneIndex] = useState<number | null>(null);

  const searchHistoryQuery = useQuery({
    queryKey: SEARCH_HISTORY_QUERY_KEY,
    queryFn: loadSearchHistory,
    initialData: loadSearchHistory,
    staleTime: Infinity,
  });

  const searchHistory = searchHistoryQuery.data;

  const searchMutation = useMutation({
    mutationFn: ({ filters, ocrTextQuery, queryFile, resultLimit }: SearchVariables) =>
      searchMedia(queryFile, resultLimit, ocrTextQuery, filters),
    onSuccess: (response, variables) => {
      const nextItem: SearchHistoryItem = {
        id: createHistoryId(),
        fileName: variables.queryFile?.name ?? `Text: ${variables.ocrTextQuery.trim()}`,
        filters: variables.filters,
        limit: variables.resultLimit,
        ocrTextQuery: variables.ocrTextQuery,
        queryImageUrl: variables.queryImageUrl,
        queryMediaKind: response.query_media_kind,
        sortMode: variables.sortMode,
        searchedAt: new Date().toISOString(),
        response,
      };

      updateSearchHistory((history) => [nextItem, ...history].slice(0, MAX_SEARCH_HISTORY));
      setActiveSearchId(nextItem.id);
      setSelectedQuerySceneIndex(response.scenes[0]?.scene_index ?? null);
    },
  });

  const faceSearchMutation = useMutation({
    mutationFn: ({ filters, queryFile, resultLimit }: FaceSearchVariables) =>
      searchFaceMedia(queryFile, resultLimit, filters),
  });
  // Face results are not stored in search history, so delete/tag edits are layered
  // over the current face search response until the next face search replaces it.
  const [faceEdits, setFaceEdits] = useState<{
    source: FaceSearchResponse;
    response: FaceSearchResponse;
  } | null>(null);
  const faceSearchData = faceSearchMutation.data;
  const faceResponse =
    faceSearchData && faceEdits?.source === faceSearchData
      ? faceEdits.response
      : (faceSearchData ?? faceEdits?.response ?? null);

  useEffect(() => {
    currentFaceSnapshot.current = faceResponse
      ? {
          source: faceSearchData ?? faceEdits?.source ?? faceResponse,
          response: faceResponse,
        }
      : null;
  }, [faceResponse, faceSearchData, faceEdits?.source]);

  const [deleteWarning, setDeleteWarning] = useState<Error | null>(null);
  // Clear only after the observer response has committed, so callbacks in the same
  // settlement batch still refresh after tag writes against the previous source.
  useEffect(() => {
    if (!faceSearchMutation.isPending) {
      faceRefreshPending.current = false;
    }
  }, [faceSearchMutation.data, faceSearchMutation.isPending]);
  const deleteMediaMutation = useMutation({
    mutationFn: async (id: string) => {
      const response = await deleteIndexedMedia(id);
      if (response.deleted_points === 0 && response.errors.length > 0) {
        throw new Error(response.errors.join("; "));
      }
      return response;
    },
    onMutate: () => {
      setDeleteWarning(null);
      return { faceGeneration: faceQueryGeneration.current };
    },
    onSuccess: (result, id, context) => {
      removeMediaFromSearchHistory(id);
      updateFaceResponse((response) => removeResultFromFaceResponse(response, id));
      if (context?.faceGeneration === faceQueryGeneration.current) {
        setDeleteWarning(result.errors.length > 0 ? new Error(result.errors.join("; ")) : null);
        // Person scores are aggregated server-side from individual faces.
        if (faceSearchMutation.variables && faceResponse) {
          refreshFaceResults();
        }
      } else {
        // The write is global; reconcile the current query without restoring the old one.
        refreshFaceResults();
      }
      // oxlint-disable typescript/no-floating-promises -- Preserve the existing detached cache refreshes after a successful mutation.
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
      // oxlint-enable typescript/no-floating-promises
    },
  });

  const updateMediaTagsMutation = useMutation({
    mutationFn: updateIndexedMediaTags,
    onMutate: () => ({ faceGeneration: faceQueryGeneration.current }),
    onSuccess: (media, _variables, context) => {
      updateMediaInSearchHistory(media);
      updateFaceResponse((response) => updateMediaInFaceResponse(response, media));
      if (context?.faceGeneration === faceQueryGeneration.current) {
        // A pending refresh may have read the index before this tag write completed.
        if (faceRefreshPending.current && faceSearchMutation.variables && faceResponse) {
          refreshFaceResults();
        }
      } else {
        // The write is global; reconcile the current query without restoring the old one.
        refreshFaceResults();
      }
      // oxlint-disable-next-line typescript/no-floating-promises -- Preserve the existing detached cache refresh after a successful mutation.
      queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
    },
  });

  // oxlint-disable react/set-state-in-effect -- Preview state is synchronized with the browser object-URL lifecycle owned by this effect.
  useEffect(() => {
    if (!file || isPdfFile(file)) {
      setPreviewUrl(null);
      return;
    }

    const url = URL.createObjectURL(file);
    setPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [file]);
  // oxlint-enable react/set-state-in-effect

  useEffect(() => {
    saveSearchHistory(searchHistory);
  }, [searchHistory]);

  const activeSearch = searchHistory.find((item) => item.id === activeSearchId) ?? null;
  const activeResponse = activeSearch?.response ?? null;
  const displayedPreviewUrl = activeSearch ? activeSearch.queryImageUrl : previewUrl;
  const previewIsText =
    activeSearch?.queryMediaKind === "text" || (!file && ocrTextQuery.trim().length > 0);
  const previewIsVideo = activeSearch
    ? activeSearch.queryMediaKind === "video"
    : Boolean(file?.type.startsWith("video/"));
  const previewIsAudio = activeSearch
    ? activeSearch.queryMediaKind === "audio"
    : Boolean(file && isAudioFile(file));
  const previewIsPdf = activeSearch
    ? activeSearch.queryMediaKind === "pdf"
    : Boolean(file && isPdfFile(file));
  const showMetadataFilters = Boolean(file || activeSearch || ocrTextQuery.trim());
  const sourceTypeOptions = sourceTypesFor(
    activeResponse?.results ?? [],
    metadataFilters.sourceType,
  );
  const filteredResults = sortResults(
    filterResults(activeResponse?.results ?? [], metadataFilters),
    resultSortMode,
  );
  const results = filteredResults.slice(0, activeSearch?.limit ?? limit);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    resetDeletionFeedback();

    if (searchMode === "face") {
      if (!file) {
        return;
      }
      setActiveSearchId(null);
      searchMutation.reset();
      faceRefreshPending.current = false;
      setFaceEdits(null);
      const variables = { filters: metadataFilters, queryFile: file, resultLimit: limit };
      activeFaceQuery.current = variables;
      faceSearchMutation.mutate(variables);
      return;
    }

    if (!file && !ocrTextQuery.trim()) {
      return;
    }

    setActiveSearchId(null);
    faceRefreshPending.current = false;
    setFaceEdits(null);
    faceSearchMutation.reset();
    const queryImageUrl = file
      ? file.type.startsWith("video/") || isAudioFile(file) || isPdfFile(file)
        ? previewUrl
        : await createQueryPreview(file)
      : null;

    searchMutation.mutate({
      filters: metadataFilters,
      ocrTextQuery,
      queryFile: file,
      queryImageUrl,
      resultLimit: limit,
      sortMode: resultSortMode,
    });
  }

  function handleFileChange(nextFile: File | null) {
    resetDeletionFeedback();
    setFile(nextFile);
    setActiveSearchId(null);
    setSelectedQuerySceneIndex(null);
    searchMutation.reset();
    faceRefreshPending.current = false;
    setFaceEdits(null);
    faceSearchMutation.reset();
  }

  function handleLimitChange(value: string) {
    const nextLimit = Number(value || DEFAULT_LIMIT);
    setLimit(nextLimit);
    updateActiveSearch((item) => ({ ...item, limit: nextLimit }));
  }

  function handleMetadataFiltersChange(nextFilters: MetadataFilters) {
    setMetadataFilters(nextFilters);
    updateActiveSearch((item) => ({ ...item, filters: nextFilters }));
  }

  function handleResultSortModeChange(sortMode: ResultSortMode) {
    setResultSortMode(sortMode);
    updateActiveSearch((item) => ({ ...item, sortMode }));
  }

  function handleHistorySelect(item: SearchHistoryItem) {
    resetDeletionFeedback();
    setActiveSearchId(item.id);
    setLimit(item.limit);
    setMetadataFilters(item.filters);
    setOcrTextQuery(item.ocrTextQuery);
    setResultSortMode(item.sortMode);
    setSelectedQuerySceneIndex(item.response.scenes[0]?.scene_index ?? null);
    searchMutation.reset();
    faceRefreshPending.current = false;
    setFaceEdits(null);
    faceSearchMutation.reset();
  }

  function applyIdentityMutationToSearchHistory(mutation: IdentityMutationResponse) {
    updateSearchHistory((history) => applyIdentityMutationToHistory(history, mutation));
  }

  function updateSearchHistory(updater: (history: SearchHistoryItem[]) => SearchHistoryItem[]) {
    queryClient.setQueryData<SearchHistoryItem[]>(SEARCH_HISTORY_QUERY_KEY, (history = []) =>
      updater(history),
    );
  }

  function updateActiveSearch(updater: (item: SearchHistoryItem) => SearchHistoryItem) {
    if (!activeSearchId) {
      return;
    }

    updateSearchHistory((history) =>
      history.map((item) => (item.id === activeSearchId ? updater(item) : item)),
    );
  }

  function refreshFaceResults() {
    const variables = activeFaceQuery.current;
    if (!variables) {
      return;
    }
    updateFaceResponse((response) => response);
    faceRefreshPending.current = true;
    faceSearchMutation.mutate(variables);
  }

  function resetDeletionFeedback() {
    faceQueryGeneration.current += 1;
    activeFaceQuery.current = null;
    currentFaceSnapshot.current = null;
    deleteMediaMutation.reset();
    setDeleteWarning(null);
  }

  function updateFaceResponse(updater: (response: FaceSearchResponse) => FaceSearchResponse) {
    const snapshot = currentFaceSnapshot.current;
    if (!snapshot) {
      return;
    }
    setFaceEdits((current) => ({
      source: snapshot.source,
      response: updater(current?.source === snapshot.source ? current.response : snapshot.response),
    }));
  }

  function removeMediaFromSearchHistory(id: string) {
    updateSearchHistory((history) =>
      history.map((item) => ({
        ...item,
        response: removeResultFromResponse(item.response, id),
      })),
    );
  }

  function updateMediaInSearchHistory(media: SearchResult["image"]) {
    updateSearchHistory((history) =>
      history.map((item) => ({
        ...item,
        response: updateMediaInResponse(item.response, media),
      })),
    );
  }

  return {
    activeResponse,
    activeSearch,
    activeSearchId,
    displayedPreviewUrl,
    deleteMediaMutation,
    file,
    handleFileChange,
    handleHistorySelect,
    handleLimitChange,
    handleMetadataFiltersChange,
    handleResultSortModeChange,
    handleSubmit,
    limit,
    metadataFilters,
    ocrTextQuery,
    previewIsAudio,
    previewIsPdf,
    previewIsText,
    previewIsVideo,
    queryClient,
    resultSortMode,
    results,
    faceResponse,
    searchError:
      deleteMediaMutation.error ??
      deleteWarning ??
      faceSearchMutation.error ??
      searchMutation.error,
    searchHistory,
    searchHistoryQuery,
    searchMutation,
    searchMode,
    searchPending:
      searchMutation.isPending || (faceSearchMutation.isPending && faceResponse === null),
    selectedQuerySceneIndex,
    setSelectedQuerySceneIndex,
    setOcrTextQuery,
    setResultSortMode,
    setSearchMode: (mode: SearchMode) => {
      if (mode === searchMode) {
        return;
      }
      resetDeletionFeedback();
      faceRefreshPending.current = false;
      setFaceEdits(null);
      faceSearchMutation.reset();
      setSearchMode(mode);
    },
    setLimit,
    setMetadataFilters,
    setActiveSearchId,
    setFile,
    setSelectedQuerySceneIndexState: setSelectedQuerySceneIndex,
    showMetadataFilters,
    sourceTypeOptions,
    updateMediaTagsMutation,
    tagSavingId: updateMediaTagsMutation.isPending
      ? updateMediaTagsMutation.variables?.id
      : undefined,
    deletePendingId: deleteMediaMutation.isPending
      ? (deleteMediaMutation.variables as string | undefined)
      : undefined,
    applyIdentityMutationToSearchHistory,
  };
}

function createHistoryId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

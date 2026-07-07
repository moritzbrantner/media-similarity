import type { FormEvent } from "react";
import { useEffect, useState } from "react";
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
  removeResultFromResponse,
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
import type { IdentityMutationResponse, SearchResult } from "../../types";

export function useSearchController() {
  const queryClient = useQueryClient();

  const [file, setFile] = useState<File | null>(null);
  const [limit, setLimit] = useState(DEFAULT_LIMIT);
  const [metadataFilters, setMetadataFilters] = useState<MetadataFilters>(DEFAULT_METADATA_FILTERS);
  const [ocrTextQuery, setOcrTextQuery] = useState("");
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
    mutationFn: ({
      filters,
      queryFile,
      resultLimit,
    }: {
      filters: MetadataFilters;
      queryFile: File;
      resultLimit: number;
    }) => searchFaceMedia(queryFile, resultLimit, filters),
  });

  const deleteMediaMutation = useMutation({
    mutationFn: deleteIndexedMedia,
    onSuccess: (_response, id) => {
      removeMediaFromSearchHistory(id);
      queryClient.invalidateQueries({ queryKey: ["health"] });
      queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
    },
  });

  const updateMediaTagsMutation = useMutation({
    mutationFn: updateIndexedMediaTags,
    onSuccess: (media) => {
      updateMediaInSearchHistory(media);
      queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
    },
  });

  useEffect(() => {
    if (!file || isPdfFile(file)) {
      setPreviewUrl(null);
      return;
    }

    const url = URL.createObjectURL(file);
    setPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [file]);

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

    if (searchMode === "face") {
      if (!file) {
        return;
      }
      setActiveSearchId(null);
      searchMutation.reset();
      faceSearchMutation.mutate({
        filters: metadataFilters,
        queryFile: file,
        resultLimit: limit,
      });
      return;
    }

    if (!file && !ocrTextQuery.trim()) {
      return;
    }

    setActiveSearchId(null);
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
    setFile(nextFile);
    setActiveSearchId(null);
    setSelectedQuerySceneIndex(null);
    searchMutation.reset();
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
    setActiveSearchId(item.id);
    setLimit(item.limit);
    setMetadataFilters(item.filters);
    setOcrTextQuery(item.ocrTextQuery);
    setResultSortMode(item.sortMode);
    setSelectedQuerySceneIndex(item.response.scenes[0]?.scene_index ?? null);
    searchMutation.reset();
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
    faceResponse: faceSearchMutation.data ?? null,
    searchError: faceSearchMutation.error ?? searchMutation.error,
    searchHistory,
    searchHistoryQuery,
    searchMutation,
    searchMode,
    searchPending: searchMutation.isPending || faceSearchMutation.isPending,
    selectedQuerySceneIndex,
    setSelectedQuerySceneIndex,
    setOcrTextQuery,
    setResultSortMode,
    setSearchMode,
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

import { useNavigate } from "react-router";
import { SearchPage } from "./search/search-page";
import { useSearchController } from "../features/search/useSearchController";
import { useAppShellContext } from "./_layout";

export function SearchRoute() {
  const navigate = useNavigate();
  const shell = useAppShellContext();
  const searchController = useSearchController();

  return (
    <SearchPage
      activeResponse={searchController.activeResponse}
      activeSearch={searchController.activeSearch}
      activeSearchId={searchController.activeSearchId}
      deletingId={searchController.deletePendingId}
      displayedPreviewUrl={searchController.displayedPreviewUrl}
      file={searchController.file}
      faceResponse={searchController.faceResponse}
      health={shell.health}
      indexError={searchController.searchError}
      lastIndex={shell.lastIndex}
      limit={searchController.limit}
      metadataFilters={searchController.metadataFilters}
      ocrTextQuery={searchController.ocrTextQuery}
      onDelete={(id) => searchController.deleteMediaMutation.mutate(id)}
      onFileChange={searchController.handleFileChange}
      onHistorySelect={searchController.handleHistorySelect}
      onLimitChange={searchController.handleLimitChange}
      onMetadataFiltersChange={searchController.handleMetadataFiltersChange}
      onOcrTextQueryChange={searchController.setOcrTextQuery}
      onResultSortModeChange={searchController.handleResultSortModeChange}
      onSearchModeChange={searchController.setSearchMode}
      onSaveAsAlbum={() => {
        shell.beginAlbumFromSearch({
          filters: searchController.metadataFilters,
          limit: searchController.limit,
          ocrTextQuery: searchController.ocrTextQuery,
          sortMode: searchController.resultSortMode,
        });
        // oxlint-disable-next-line typescript/no-floating-promises -- Preserve the existing fire-and-forget route transition from this UI event.
        navigate("/albums");
      }}
      onSearchSubmit={searchController.handleSubmit}
      onSelectQueryScene={searchController.setSelectedQuerySceneIndex}
      onUpdateTags={(id, tags) =>
        searchController.updateMediaTagsMutation.mutate({
          id,
          tags,
        })
      }
      previewIsAudio={searchController.previewIsAudio}
      previewIsPdf={searchController.previewIsPdf}
      previewIsText={searchController.previewIsText}
      previewIsVideo={searchController.previewIsVideo}
      resultSortMode={searchController.resultSortMode}
      results={searchController.results}
      searchError={searchController.searchError}
      searchHistory={searchController.searchHistory}
      searchMode={searchController.searchMode}
      searchPending={searchController.searchPending}
      selectedQuerySceneIndex={searchController.selectedQuerySceneIndex}
      showMetadataFilters={searchController.showMetadataFilters}
      sourceTypeOptions={searchController.sourceTypeOptions}
      tagSavingId={searchController.tagSavingId}
    />
  );
}

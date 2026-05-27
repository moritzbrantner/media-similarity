import type { FormEvent } from "react";

import { MetadataFiltersPanel, ResultSortSelect } from "../components/filter-fields";
import { QueryMediaForm } from "../components/query-media-form";
import { QueryPreviewPanel } from "../components/query-preview-panel";
import { ResultsGrid } from "../components/results-grid";
import { SceneResultsList } from "../components/scene-results-list";
import { SearchHistoryList } from "../components/search-history-list";
import type { MetadataFilters, ResultSortMode, SearchHistoryItem } from "../search/types";
import type { HealthResponse, IndexResponse, SearchResponse, SearchResult } from "../types";

type SearchPageProps = {
  activeResponse: SearchResponse | null;
  activeSearch: SearchHistoryItem | null;
  activeSearchId: string | null;
  deletingId: string | undefined;
  displayedPreviewUrl: string | null;
  file: File | null;
  health: HealthResponse | undefined;
  indexError: Error | null;
  lastIndex: IndexResponse | null;
  limit: number;
  metadataFilters: MetadataFilters;
  ocrTextQuery: string;
  onDelete: (id: string) => void;
  onFileChange: (file: File | null) => void;
  onHistorySelect: (item: SearchHistoryItem) => void;
  onLimitChange: (value: string) => void;
  onMetadataFiltersChange: (filters: MetadataFilters) => void;
  onOcrTextQueryChange: (value: string) => void;
  onResultSortModeChange: (mode: ResultSortMode) => void;
  onSaveAsAlbum: () => void;
  onSearchSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onSelectQueryScene: (sceneIndex: number | null) => void;
  onUpdateTags: (id: string, tags: string[]) => void;
  previewIsAudio: boolean;
  previewIsPdf: boolean;
  previewIsVideo: boolean;
  resultSortMode: ResultSortMode;
  results: SearchResult[];
  searchError: Error | null;
  searchHistory: SearchHistoryItem[];
  searchPending: boolean;
  selectedQuerySceneIndex: number | null;
  showMetadataFilters: boolean;
  sourceTypeOptions: string[];
  tagSavingId: string | undefined;
};

export function SearchPage({
  activeResponse,
  activeSearch,
  activeSearchId,
  deletingId,
  displayedPreviewUrl,
  file,
  health,
  indexError,
  lastIndex,
  limit,
  metadataFilters,
  ocrTextQuery,
  onDelete,
  onFileChange,
  onHistorySelect,
  onLimitChange,
  onMetadataFiltersChange,
  onOcrTextQueryChange,
  onResultSortModeChange,
  onSaveAsAlbum,
  onSearchSubmit,
  onSelectQueryScene,
  onUpdateTags,
  previewIsAudio,
  previewIsPdf,
  previewIsVideo,
  resultSortMode,
  results,
  searchError,
  searchHistory,
  searchPending,
  selectedQuerySceneIndex,
  showMetadataFilters,
  sourceTypeOptions,
  tagSavingId,
}: SearchPageProps) {
  return (
    <>
      <section className="grid gap-5 lg:grid-cols-[360px_minmax(0,1fr)]">
        <QueryMediaForm
          file={file}
          indexError={indexError}
          lastIndex={lastIndex}
          limit={limit}
          ocrTextQuery={ocrTextQuery}
          onFileChange={onFileChange}
          onLimitChange={onLimitChange}
          onOcrTextQueryChange={onOcrTextQueryChange}
          onSubmit={onSearchSubmit}
          searchError={searchError}
          searchPending={searchPending}
        />

        <QueryPreviewPanel
          previewIsAudio={previewIsAudio}
          previewIsPdf={previewIsPdf}
          previewIsVideo={previewIsVideo}
          previewUrl={displayedPreviewUrl}
        />
      </section>

      {showMetadataFilters ? (
        <MetadataFiltersPanel
          filters={metadataFilters}
          onChange={onMetadataFiltersChange}
          onSaveAsAlbum={onSaveAsAlbum}
          ocrTextQuery={ocrTextQuery}
          sourceTypeOptions={sourceTypeOptions}
        />
      ) : null}

      <section className="grid gap-5 lg:grid-cols-[280px_minmax(0,1fr)]">
        <SearchHistoryList
          activeSearchId={activeSearchId}
          history={searchHistory}
          onSelect={onHistorySelect}
        />

        <div className="flex min-w-0 flex-col gap-3">
          <div className="flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <h2 className="text-lg font-semibold text-neutral-950">Results</h2>
              <p className="text-sm text-neutral-600">
                {activeResponse?.scenes.length
                  ? `${activeResponse.scenes.length} scene(s), ${results.length} unique result(s)`
                  : activeResponse
                    ? `${results.length} of ${activeResponse.count} result(s), query pHash ${activeResponse.query_phash}`
                    : searchPending
                      ? "Searching indexed media."
                      : "Search results will appear here."}
              </p>
            </div>
            {health ? (
              <span className="truncate text-sm text-neutral-600" title={health.collection}>
                Collection: {health.collection}
              </span>
            ) : null}
            <ResultSortSelect onChange={onResultSortModeChange} value={resultSortMode} />
          </div>

          {activeResponse?.scenes.length ? (
            <SceneResultsList
              deletingId={deletingId}
              filters={metadataFilters}
              onDelete={onDelete}
              onUpdateTags={onUpdateTags}
              onSelectScene={onSelectQueryScene}
              scenes={activeResponse.scenes}
              selectedSceneIndex={selectedQuerySceneIndex}
              resultLimit={activeSearch?.limit ?? limit}
              sortMode={resultSortMode}
              tagSavingId={tagSavingId}
            />
          ) : (
            <ResultsGrid
              pending={searchPending}
              results={results}
              searched={Boolean(activeResponse)}
              deletingId={deletingId}
              onDelete={onDelete}
              onUpdateTags={onUpdateTags}
              tagSavingId={tagSavingId}
            />
          )}
        </div>
      </section>
    </>
  );
}

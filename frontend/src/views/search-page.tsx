import type { FormEvent } from "react";

import { MetadataFiltersPanel, ResultSortSelect } from "../components/filter-fields";
import { QueryMediaForm } from "../components/query-media-form";
import { QueryPreviewPanel } from "../components/query-preview-panel";
import { ResultsGrid } from "../components/results-grid";
import { SceneResultsList } from "../components/scene-results-list";
import { SearchHistoryList } from "../components/search-history-list";
import type {
  MetadataFilters,
  ResultSortMode,
  SearchHistoryItem,
  SearchMode,
} from "../search/types";
import type {
  FaceSearchResponse,
  HealthResponse,
  IndexResponse,
  SearchResponse,
  SearchResult,
} from "../types";

type SearchPageProps = {
  activeResponse: SearchResponse | null;
  activeSearch: SearchHistoryItem | null;
  activeSearchId: string | null;
  deletingId: string | undefined;
  displayedPreviewUrl: string | null;
  faceResponse: FaceSearchResponse | null;
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
  onSearchModeChange: (mode: SearchMode) => void;
  onSaveAsAlbum: () => void;
  onSearchSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onSelectQueryScene: (sceneIndex: number | null) => void;
  onUpdateTags: (id: string, tags: string[]) => void;
  previewIsAudio: boolean;
  previewIsPdf: boolean;
  previewIsText: boolean;
  previewIsVideo: boolean;
  resultSortMode: ResultSortMode;
  results: SearchResult[];
  searchError: Error | null;
  searchHistory: SearchHistoryItem[];
  searchMode: SearchMode;
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
  faceResponse,
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
  onSearchModeChange,
  onSaveAsAlbum,
  onSearchSubmit,
  onSelectQueryScene,
  onUpdateTags,
  previewIsAudio,
  previewIsPdf,
  previewIsText,
  previewIsVideo,
  resultSortMode,
  results,
  searchError,
  searchHistory,
  searchMode,
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
          onSearchModeChange={onSearchModeChange}
          onSubmit={onSearchSubmit}
          searchError={searchError}
          searchMode={searchMode}
          searchPending={searchPending}
        />

        <QueryPreviewPanel
          previewIsAudio={previewIsAudio}
          previewIsPdf={previewIsPdf}
          previewIsText={previewIsText}
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
                {searchMode === "face" && faceResponse
                  ? `${faceResponse.people.length} people, ${faceResponse.results.length} media match(es)`
                  : activeResponse?.scenes.length
                    ? `${activeResponse.scenes.length} scene(s), ${results.length} unique result(s)`
                    : activeResponse
                      ? activeResponse.query_media_kind === "text"
                        ? `${results.length} of ${activeResponse.count} text result(s)`
                        : `${results.length} of ${activeResponse.count} result(s), query pHash ${activeResponse.query_phash}`
                      : searchPending
                        ? "Searching indexed media."
                        : "Search results will appear here."}
              </p>
              {activeResponse?.query_visual_embedding_degraded ? (
                <p className="text-sm font-medium text-amber-700">
                  Visual embedding is running in degraded mode
                  {activeResponse.query_visual_embedding_model
                    ? ` (${activeResponse.query_visual_embedding_model})`
                    : ""}
                  .
                </p>
              ) : null}
              {faceResponse?.query.model_status.degraded ? (
                <p className="text-sm font-medium text-amber-700">
                  Face search is running in degraded mode.
                </p>
              ) : null}
            </div>
            {health ? (
              <span className="truncate text-sm text-neutral-600" title={health.collection}>
                Collection: {health.collection}
              </span>
            ) : null}
            <ResultSortSelect onChange={onResultSortModeChange} value={resultSortMode} />
          </div>

          {searchMode === "face" ? (
            <FaceSearchResults
              deletingId={deletingId}
              faceResponse={faceResponse}
              onDelete={onDelete}
              onUpdateTags={onUpdateTags}
              pending={searchPending}
              tagSavingId={tagSavingId}
            />
          ) : activeResponse?.scenes.length ? (
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

function FaceSearchResults({
  deletingId,
  faceResponse,
  onDelete,
  onUpdateTags,
  pending,
  tagSavingId,
}: {
  deletingId: string | undefined;
  faceResponse: FaceSearchResponse | null;
  onDelete: (id: string) => void;
  onUpdateTags: (id: string, tags: string[]) => void;
  pending: boolean;
  tagSavingId: string | undefined;
}) {
  const mediaResults = faceResponse?.results.map((match) => match.result) ?? [];
  return (
    <div className="flex flex-col gap-4">
      {faceResponse?.people.length ? (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {faceResponse.people.map((person) => (
            <div
              className="rounded-lg border border-neutral-300 bg-white p-3 shadow-sm"
              key={person.person_id}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <h3 className="truncate text-sm font-semibold text-neutral-950">
                    {person.label?.trim() || person.person_id}
                  </h3>
                  <p className="truncate text-xs text-neutral-500" title={person.person_id}>
                    {person.person_id}
                  </p>
                </div>
                <span className="shrink-0 rounded bg-emerald-50 px-2 py-1 text-xs font-semibold text-emerald-900">
                  {person.score.toFixed(4)}
                </span>
              </div>
              <p className="mt-2 text-xs text-neutral-600">
                {person.face_count} face(s) across {person.media_count} media item(s)
              </p>
            </div>
          ))}
        </div>
      ) : null}
      <ResultsGrid
        deletingId={deletingId}
        onDelete={onDelete}
        onUpdateTags={onUpdateTags}
        pending={pending}
        results={mediaResults}
        searched={Boolean(faceResponse)}
        tagSavingId={tagSavingId}
      />
    </div>
  );
}

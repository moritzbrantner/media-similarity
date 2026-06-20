import { useState } from "react";
import type { EditableSmartAlbum } from "../../types";
import { smartAlbumDraftFromSearch } from "../../components/smart-albums-page";
import type { MetadataFilters, ResultSortMode } from "../../search/types";

export function useAlbumController() {
  const [albumDraft, setAlbumDraft] = useState<EditableSmartAlbum | null>(null);

  function beginAlbumFromSearch(params: {
    filters: MetadataFilters;
    limit: number;
    ocrTextQuery: string;
    sortMode: ResultSortMode;
  }) {
    setAlbumDraft(
      smartAlbumDraftFromSearch({
        filters: params.filters,
        limit: params.limit,
        ocrTextQuery: params.ocrTextQuery,
        sortMode: params.sortMode,
      }),
    );
  }

  function consumeAlbumDraft() {
    setAlbumDraft(null);
  }

  return {
    albumDraft,
    beginAlbumFromSearch,
    consumeAlbumDraft,
    setAlbumDraft,
  };
}

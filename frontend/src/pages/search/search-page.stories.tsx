import type { Meta, StoryObj } from "@storybook/react-vite";
import { DEFAULT_METADATA_FILTERS } from "../../search/defaults";
import type { SearchHistoryItem } from "../../search/types";
import { faceSearchResponse, healthResponse, searchResponse } from "../../testing/media-fixtures";
import { SearchPage } from "./search-page";

const historyItem: SearchHistoryItem = {
  fileName: "query.png",
  filters: DEFAULT_METADATA_FILTERS,
  id: "history-1",
  limit: 24,
  ocrTextQuery: "",
  queryImageUrl: null,
  queryMediaKind: "static_image",
  response: searchResponse,
  searchedAt: "2026-05-22T10:00:00Z",
  sortMode: "relevance",
};

const meta = {
  component: SearchPage,
  args: {
    activeResponse: searchResponse,
    activeSearch: historyItem,
    activeSearchId: historyItem.id,
    deletingId: undefined,
    displayedPreviewUrl: null,
    faceResponse: null,
    file: null,
    health: healthResponse,
    indexError: null,
    lastIndex: null,
    limit: 24,
    metadataFilters: DEFAULT_METADATA_FILTERS,
    ocrTextQuery: "",
    onDelete: () => undefined,
    onFileChange: () => undefined,
    onHistorySelect: () => undefined,
    onLimitChange: () => undefined,
    onMetadataFiltersChange: () => undefined,
    onOcrTextQueryChange: () => undefined,
    onResultSortModeChange: () => undefined,
    onSaveAsAlbum: () => undefined,
    onSearchModeChange: () => undefined,
    onSearchSubmit: (event) => event.preventDefault(),
    onSelectQueryScene: () => undefined,
    onUpdateTags: () => undefined,
    previewIsAudio: false,
    previewIsPdf: false,
    previewIsText: false,
    previewIsVideo: false,
    resultSortMode: "relevance",
    results: searchResponse.results,
    searchError: null,
    searchHistory: [historyItem],
    searchMode: "media",
    searchPending: false,
    selectedQuerySceneIndex: null,
    showMetadataFilters: true,
    sourceTypeOptions: ["local", "import"],
    tagSavingId: undefined,
  },
} satisfies Meta<typeof SearchPage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Results: Story = {};

export const Pending: Story = {
  args: {
    activeResponse: null,
    results: [],
    searchPending: true,
  },
};

export const FaceResults: Story = {
  args: {
    activeResponse: null,
    faceResponse: faceSearchResponse,
    results: [],
    searchMode: "face",
  },
};

export const ErrorState: Story = {
  args: {
    activeResponse: null,
    results: [],
    searchError: new Error("Search failed."),
  },
};

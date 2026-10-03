import type { Meta, StoryObj } from "@storybook/react-vite";
import { DEFAULT_METADATA_FILTERS } from "../../../search/defaults";
import { searchResponse } from "../../../testing/media-fixtures";
import { SearchHistoryList } from "./search-history-list";

const history = [
  {
    fileName: "query.png",
    filters: DEFAULT_METADATA_FILTERS,
    id: "search-1",
    limit: 24,
    ocrTextQuery: "",
    queryImageUrl: null,
    queryMediaKind: "static_image",
    response: searchResponse,
    searchedAt: "2026-05-22T10:00:00Z",
    sortMode: "relevance",
  },
] satisfies Parameters<typeof SearchHistoryList>[0]["history"];

const meta = {
  component: SearchHistoryList,
  args: {
    activeSearchId: "search-1",
    history,
    onSelect: () => undefined,
  },
} satisfies Meta<typeof SearchHistoryList>;

export default meta;

type Story = StoryObj<typeof meta>;

export const WithHistory: Story = {};

export const Empty: Story = {
  args: {
    activeSearchId: null,
    history: [],
  },
};

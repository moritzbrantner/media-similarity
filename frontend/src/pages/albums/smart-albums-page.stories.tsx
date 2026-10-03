import type { Meta, StoryObj } from "@storybook/react-vite";
import { smartAlbum, smartAlbumResults } from "../../testing/media-fixtures";
import { SmartAlbumsPage } from "./smart-albums-page";

const apiMocks = [
  { response: { albums: [smartAlbum] }, url: "/api/smart-albums" },
  { response: smartAlbumResults, url: "/api/smart-albums/album-sunrise/results" },
  { method: "POST", response: smartAlbumResults, url: "/api/smart-albums/preview" },
];

const meta = {
  component: SmartAlbumsPage,
  args: {
    initialDraft: null,
    onDraftConsumed: () => undefined,
  },
  parameters: {
    apiMocks,
  },
} satisfies Meta<typeof SmartAlbumsPage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Loaded: Story = {};

export const Empty: Story = {
  parameters: {
    apiMocks: [{ response: { albums: [] }, url: "/api/smart-albums" }],
  },
};

export const InitialDraft: Story = {
  args: {
    initialDraft: {
      criteria: smartAlbum.criteria,
      description: "Images matching the current query filters",
      limit: 24,
      name: "Search album draft",
      sort: "modified_newest",
    },
  },
};

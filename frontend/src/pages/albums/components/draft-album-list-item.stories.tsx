import type { Meta, StoryObj } from "@storybook/react-vite";
import { smartAlbum } from "../../../testing/media-fixtures";
import { DraftAlbumListItem } from "./draft-album-list-item";

const meta = {
  component: DraftAlbumListItem,
  args: {
    draft: {
      criteria: smartAlbum.criteria,
      description: "Built from current search filters",
      limit: 24,
      name: "Recent portraits",
      sort: "modified_newest",
    },
  },
} satisfies Meta<typeof DraftAlbumListItem>;

export default meta;

type Story = StoryObj<typeof meta>;

export const NamedDraft: Story = {};

export const UntitledDraft: Story = {
  args: {
    draft: {
      criteria: smartAlbum.criteria,
      description: "",
      limit: 24,
      name: "",
      sort: "modified_newest",
    },
  },
};

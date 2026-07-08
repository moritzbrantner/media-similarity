import type { Meta, StoryObj } from "@storybook/react-vite";
import { searchResponse } from "../testing/media-fixtures";
import { ResultsGrid } from "./results-grid";

const meta = {
  component: ResultsGrid,
  args: {
    deletingId: undefined,
    onDelete: () => undefined,
    onUpdateTags: () => undefined,
    pending: false,
    results: searchResponse.results,
    searched: true,
    tagSavingId: undefined,
  },
} satisfies Meta<typeof ResultsGrid>;

export default meta;

type Story = StoryObj<typeof meta>;

export const WithResults: Story = {};

export const Loading: Story = {
  args: {
    pending: true,
  },
};

export const EmptyBeforeSearch: Story = {
  args: {
    results: [],
    searched: false,
  },
};

export const EmptyAfterSearch: Story = {
  args: {
    results: [],
    searched: true,
  },
};

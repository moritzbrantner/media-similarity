import type { Meta, StoryObj } from "@storybook/react-vite";
import { inverseIndexResponse } from "../../testing/media-fixtures";
import { InverseIndexPage } from "./inverse-index-page";

const meta = {
  component: InverseIndexPage,
  args: {
    data: inverseIndexResponse,
    error: null,
    loading: false,
    mergeError: null,
    mergeErrorIdentity: null,
    mergingIdentity: null,
    onMergeIdentity: async () => undefined,
    onRefresh: () => undefined,
    onRenameIdentity: async () => undefined,
    refreshing: false,
    renameError: null,
    renameErrorIdentity: null,
    renamingIdentity: null,
  },
} satisfies Meta<typeof InverseIndexPage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Loaded: Story = {};

export const Loading: Story = {
  args: {
    data: null,
    loading: true,
  },
};

export const Empty: Story = {
  args: {
    data: {
      ...inverseIndexResponse,
      people: [],
      speakers: [],
    },
  },
};

export const ErrorState: Story = {
  args: {
    data: null,
    error: new Error("Inverse index unavailable."),
  },
};

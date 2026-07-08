import type { Meta, StoryObj } from "@storybook/react-vite";
import { indexResponse, sourceConfigResponse } from "../../testing/media-fixtures";
import { IndexingConfigurationPage } from "./indexing-configuration-page";

const meta = {
  component: IndexingConfigurationPage,
  args: {
    config: sourceConfigResponse,
    error: null,
    indexError: null,
    indexPending: false,
    lastIndex: indexResponse,
    loading: false,
    onIndex: () => undefined,
    onSave: () => undefined,
    saveError: null,
    savePending: false,
    saveSuccess: false,
  },
} satisfies Meta<typeof IndexingConfigurationPage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Loaded: Story = {};

export const Saving: Story = {
  args: {
    savePending: true,
  },
};

export const ErrorState: Story = {
  args: {
    config: null,
    error: new Error("Indexing configuration unavailable."),
  },
};

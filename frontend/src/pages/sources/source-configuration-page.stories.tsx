import type { Meta, StoryObj } from "@storybook/react-vite";
import { indexResponse, modelsResponse, sourceConfigResponse } from "../../testing/media-fixtures";
import { SourceConfigurationPage } from "./source-configuration-page";

const meta = {
  component: SourceConfigurationPage,
  args: {
    config: sourceConfigResponse,
    error: null,
    indexError: null,
    indexPending: false,
    lastIndex: indexResponse,
    loading: false,
    modelActionPending: undefined,
    modelError: null,
    models: modelsResponse,
    modelsError: null,
    modelsLoading: false,
    onDisableModel: () => undefined,
    onDownloadAllModels: () => undefined,
    onDownloadModel: () => undefined,
    onEnableModel: () => undefined,
    onIndex: () => undefined,
    onPreview: () => undefined,
    onSave: () => undefined,
    preview: null,
    previewError: null,
    previewPending: false,
    saveError: null,
    savePending: false,
    saveSuccess: false,
  },
} satisfies Meta<typeof SourceConfigurationPage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Loaded: Story = {};

export const Loading: Story = {
  args: {
    loading: true,
  },
};

export const ReadOnly: Story = {
  args: {
    config: { ...sourceConfigResponse, media_sources_writable: false },
  },
};

export const ErrorState: Story = {
  args: {
    config: null,
    error: new Error("Source configuration unavailable."),
  },
};

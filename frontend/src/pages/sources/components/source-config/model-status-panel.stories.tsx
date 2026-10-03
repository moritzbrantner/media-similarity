import type { Meta, StoryObj } from "@storybook/react-vite";
import { modelsResponse } from "../../../../testing/media-fixtures";
import { ModelStatusPanel } from "./model-status-panel";

const meta = {
  component: ModelStatusPanel,
  args: {
    actionPendingRole: undefined,
    degradedRoles: ["audio_transcription"],
    error: null,
    loading: false,
    models: modelsResponse.models,
    onDisable: () => undefined,
    onDownload: () => undefined,
    onDownloadAll: () => undefined,
    onEnable: () => undefined,
    requiredRoles: ["visual_embedding", "audio_transcription"],
  },
} satisfies Meta<typeof ModelStatusPanel>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Loaded: Story = {};

export const Loading: Story = {
  args: {
    loading: true,
  },
};

export const ErrorState: Story = {
  args: {
    error: new Error("Model registry unavailable."),
  },
};

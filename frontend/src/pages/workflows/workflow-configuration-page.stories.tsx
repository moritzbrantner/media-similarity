import type { Meta, StoryObj } from "@storybook/react-vite";
import { indexResponse, workflowConfigResponse } from "../../testing/media-fixtures";
import { WorkflowConfigurationPage } from "./workflow-configuration-page";

const meta = {
  component: WorkflowConfigurationPage,
  args: {
    config: workflowConfigResponse,
    error: null,
    indexError: null,
    indexPending: false,
    lastIndex: indexResponse,
    loading: false,
    onIndex: () => undefined,
    onReset: () => undefined,
    onSave: () => undefined,
    onValidate: async () => [],
    resetPending: false,
    saveError: null,
    savePending: false,
    saveSuccess: false,
    validateError: null,
    validatePending: false,
  },
} satisfies Meta<typeof WorkflowConfigurationPage>;

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
    config: { ...workflowConfigResponse, writable: false },
  },
};

export const ErrorState: Story = {
  args: {
    config: null,
    error: new Error("Workflow configuration unavailable."),
  },
};

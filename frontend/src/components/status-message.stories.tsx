import type { Meta, StoryObj } from "@storybook/react-vite";
import { AlertCircle } from "lucide-react";
import { indexResponse } from "../testing/media-fixtures";
import { Message, StatusMessage } from "./status-message";

const meta = {
  component: StatusMessage,
  args: {
    indexError: null,
    lastIndex: null,
    searchError: null,
    searchPending: false,
  },
} satisfies Meta<typeof StatusMessage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Ready: Story = {};

export const Searching: Story = {
  args: {
    searchPending: true,
  },
};

export const SearchError: Story = {
  args: {
    searchError: new Error("Search service unavailable."),
  },
};

export const IndexComplete: Story = {
  args: {
    lastIndex: indexResponse,
  },
};

export const MessageWarning: StoryObj<typeof Message> = {
  render: () => (
    <Message
      icon={<AlertCircle className="size-4" />}
      text="Visual embedding is running in degraded mode."
      tone="warn"
    />
  ),
};

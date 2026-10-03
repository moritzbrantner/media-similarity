import type { Meta, StoryObj } from "@storybook/react-vite";
import { AppHeader } from "./app-header";
import type { HealthResponse } from "../types";

const health: HealthResponse = {
  collection: "media",
  source_dir: "/media/pictures",
  sources: ["/media/pictures", "/archive"],
  status: "ok",
};

const meta = {
  component: AppHeader,
  args: {
    health,
    healthError: false,
    healthLoading: false,
    indexActive: false,
    indexPending: false,
    onIndex: () => undefined,
    sourcesLabel: "/media/pictures, /archive",
  },
} satisfies Meta<typeof AppHeader>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SearchActive: Story = {
  parameters: {
    route: "/",
  },
};

export const SourcesActive: Story = {
  parameters: {
    route: "/sources",
  },
};

export const IndexRunning: Story = {
  args: {
    indexActive: true,
  },
  parameters: {
    route: "/workflows",
  },
};

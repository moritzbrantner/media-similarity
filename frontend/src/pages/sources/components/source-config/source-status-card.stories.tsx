import type { Meta, StoryObj } from "@storybook/react-vite";
import type { SourceInventory } from "../../../../types";
import { sourceConfigResponse } from "../../../../testing/media-fixtures";
import { SourceStatusCard } from "./source-status-card";

const inventory = {
  degraded_model_roles: ["visual_embedding"],
  extension_counts: { ".jpg": 4, ".png": 2 },
  media_kind_counts: { static_image: 6 },
  required_model_roles: ["visual_embedding"],
  sample_items: [],
  scanned_count: 6,
  truncated: false,
} satisfies SourceInventory;

const meta = {
  component: SourceStatusCard,
  args: {
    inventory,
    source: sourceConfigResponse.sources[0],
  },
} satisfies Meta<typeof SourceStatusCard>;

export default meta;

type Story = StoryObj<typeof meta>;

export const ReadyWithInventory: Story = {};

export const Degraded: Story = {
  args: {
    source: {
      ...sourceConfigResponse.sources[0],
      detail: "Visual embedding model is unavailable.",
      status: "degraded",
    },
  },
};

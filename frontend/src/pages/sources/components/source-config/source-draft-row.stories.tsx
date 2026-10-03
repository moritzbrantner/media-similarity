import type { Meta, StoryObj } from "@storybook/react-vite";
import { sourceConfigResponse } from "../../../../testing/media-fixtures";
import { SourceDraftRow } from "./source-draft-row";

const meta = {
  component: SourceDraftRow,
  args: {
    index: 0,
    onRemove: () => undefined,
    onUpdate: () => undefined,
    source: { id: "draft-1", kind: "local", spec: "/images" },
    supportedTypes: sourceConfigResponse.supported_source_types,
  },
} satisfies Meta<typeof SourceDraftRow>;

export default meta;

type Story = StoryObj<typeof meta>;

export const LocalSource: Story = {};

export const PlannedSource: Story = {
  args: {
    source: { id: "draft-2", kind: "camera", spec: "camera://front-door" },
  },
};

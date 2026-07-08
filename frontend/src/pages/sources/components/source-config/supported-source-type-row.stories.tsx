import type { Meta, StoryObj } from "@storybook/react-vite";
import { sourceConfigResponse } from "../../../../testing/media-fixtures";
import { SupportedSourceTypeRow } from "./supported-source-type-row";

const meta = {
  component: SupportedSourceTypeRow,
  args: {
    sourceType: sourceConfigResponse.supported_source_types[0],
  },
} satisfies Meta<typeof SupportedSourceTypeRow>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Available: Story = {};

export const Planned: Story = {
  args: {
    sourceType: sourceConfigResponse.supported_source_types[3],
  },
};

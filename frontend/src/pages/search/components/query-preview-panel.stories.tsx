import type { Meta, StoryObj } from "@storybook/react-vite";
import { pngPixelDataUrl } from "../../../testing/media-fixtures";
import { QueryPreviewPanel } from "./query-preview-panel";

const meta = {
  component: QueryPreviewPanel,
  args: {
    previewIsAudio: false,
    previewIsPdf: false,
    previewIsText: false,
    previewIsVideo: false,
    previewUrl: null,
  },
} satisfies Meta<typeof QueryPreviewPanel>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

export const ImagePreview: Story = {
  args: {
    previewUrl: pngPixelDataUrl,
  },
};

export const TextQuery: Story = {
  args: {
    previewIsText: true,
  },
};

export const PdfQuery: Story = {
  args: {
    previewIsPdf: true,
  },
};

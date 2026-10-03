import type { Meta, StoryObj } from "@storybook/react-vite";
import { PhotoMetadataDetails } from "./PhotoMetadataDetails";
import type { PhotoMetadata } from "./result-formatting";

const metadata = {
  camera_make: "Fujifilm",
  camera_model: "X-T5",
  capture_time: "2026-08-23T11:42:00Z",
  copyright: null,
  creator: "Example photographer",
  description: null,
  gps: {
    altitude_meters: 247.3,
    latitude: 48.77585,
    longitude: 9.18293,
  },
  keywords: ["outdoors", "portrait"],
  lens_model: "XF 35mm F2 R WR",
  orientation: "Horizontal",
  rating: 5,
  raw: [
    {
      key: "Make",
      label: "Camera make",
      namespace: "exif",
      value: "Fujifilm",
    },
    {
      key: "Model",
      label: "Camera model",
      namespace: "exif",
      value: "X-T5",
    },
    {
      key: "Creator",
      label: "Creator",
      namespace: "iptc",
      value: "Example photographer",
    },
  ],
  title: "Afternoon portrait",
} satisfies PhotoMetadata;

const meta = {
  component: PhotoMetadataDetails,
  args: {
    metadata,
  },
} satisfies Meta<typeof PhotoMetadataDetails>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Populated: Story = {};

export const EmptyRawMetadata: Story = {
  args: {
    metadata: {
      ...metadata,
      raw: [],
    },
  },
};

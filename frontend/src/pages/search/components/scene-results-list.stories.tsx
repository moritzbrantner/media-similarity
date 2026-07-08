import type { Meta, StoryObj } from "@storybook/react-vite";
import { DEFAULT_METADATA_FILTERS } from "../../../search/defaults";
import { makeScene, searchResponse } from "../../../testing/media-fixtures";
import { SceneResultsList } from "./scene-results-list";

const scenes = [
  makeScene({
    count: 1,
    end_frame: 120,
    end_seconds: 5,
    results: [searchResponse.results[0]],
    scene_index: 0,
    start_frame: 0,
    start_seconds: 0,
  }),
  makeScene({
    count: 1,
    end_frame: 240,
    end_seconds: 10,
    results: [searchResponse.results[1]],
    scene_index: 1,
    start_frame: 121,
    start_seconds: 5,
  }),
];

const meta = {
  component: SceneResultsList,
  args: {
    deletingId: undefined,
    filters: DEFAULT_METADATA_FILTERS,
    onDelete: () => undefined,
    onSelectScene: () => undefined,
    onUpdateTags: () => undefined,
    resultLimit: 12,
    scenes,
    selectedSceneIndex: 0,
    sortMode: "relevance",
    tagSavingId: undefined,
  },
} satisfies Meta<typeof SceneResultsList>;

export default meta;

type Story = StoryObj<typeof meta>;

export const VideoScenes: Story = {};

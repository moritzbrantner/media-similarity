import type { Meta, StoryObj } from "@storybook/react-vite";
import { makeResult, searchResponse } from "../../testing/media-fixtures";
import { ResultCard } from "./result-card";

const taggedResult = makeResult({
  image: {
    ...searchResponse.results[0].image,
    tags: ["portrait", "favorite"],
  },
});

const meta = {
  component: ResultCard,
  args: {
    deleting: false,
    onDelete: () => undefined,
    onUpdateTags: () => undefined,
    result: taggedResult,
    tagSaving: false,
  },
} satisfies Meta<typeof ResultCard>;

export default meta;

type Story = StoryObj<typeof meta>;

export const ImageResult: Story = {};

export const Deleting: Story = {
  args: {
    deleting: true,
  },
};

export const WithoutActions: Story = {
  args: {
    onDelete: undefined,
    onUpdateTags: undefined,
  },
};

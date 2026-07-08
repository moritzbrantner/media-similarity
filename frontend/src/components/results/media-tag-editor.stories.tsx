import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { searchResponse } from "../../testing/media-fixtures";
import type { SearchResult } from "../../types";
import { MediaTagEditor } from "./media-tag-editor";

function EditableTags({ saving = false }: { saving?: boolean }) {
  const [image, setImage] = useState<SearchResult["image"]>({
    ...searchResponse.results[0].image,
    tags: ["portrait", "favorite"],
  });

  return (
    <MediaTagEditor
      image={image}
      onUpdateTags={(_id, tags) => setImage((current) => ({ ...current, tags }))}
      saving={saving}
    />
  );
}

const meta = {
  component: MediaTagEditor,
  args: {
    image: {
      ...searchResponse.results[0].image,
      tags: ["portrait", "favorite"],
    },
    onUpdateTags: () => undefined,
    saving: false,
  },
} satisfies Meta<typeof MediaTagEditor>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Editable: Story = {
  render: () => <EditableTags />,
};

export const Saving: Story = {
  render: () => <EditableTags saving />,
};

export const ReadOnly: Story = {
  args: {
    onUpdateTags: undefined,
  },
};

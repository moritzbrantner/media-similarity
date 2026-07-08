import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { DEFAULT_METADATA_FILTERS } from "../../../search/defaults";
import type { MetadataFilters } from "../../../search/types";
import { MetadataFiltersPanel, ResultSortSelect } from "./filter-fields";

function StatefulFilters() {
  const [filters, setFilters] = useState<MetadataFilters>({
    ...DEFAULT_METADATA_FILTERS,
    mediaKind: "static_image",
    nameQuery: "sunrise",
    nearDuplicate: "only",
  });

  return (
    <MetadataFiltersPanel
      filters={filters}
      ocrTextQuery=""
      onChange={setFilters}
      onSaveAsAlbum={() => undefined}
      sourceTypeOptions={["local", "import", "s3"]}
    />
  );
}

const meta = {
  component: MetadataFiltersPanel,
  args: {
    filters: DEFAULT_METADATA_FILTERS,
    onChange: () => undefined,
    onSaveAsAlbum: () => undefined,
    ocrTextQuery: "",
    sourceTypeOptions: ["local", "import", "s3"],
  },
} satisfies Meta<typeof MetadataFiltersPanel>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Filters: Story = {
  render: () => <StatefulFilters />,
};

export const SortSelect: StoryObj<typeof ResultSortSelect> = {
  render: () => <ResultSortSelect onChange={() => undefined} value="relevance" />,
};

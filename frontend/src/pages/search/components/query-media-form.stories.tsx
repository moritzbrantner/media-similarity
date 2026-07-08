import type { Meta, StoryObj } from "@storybook/react-vite";
import type { FormEvent } from "react";
import { useState } from "react";
import { indexResponse } from "../../../testing/media-fixtures";
import type { SearchMode } from "../../../search/types";
import { QueryMediaForm } from "./query-media-form";

function StatefulForm({ pending = false }: { pending?: boolean }) {
  const [file, setFile] = useState<File | null>({ name: "query.png" } as File);
  const [limit, setLimit] = useState(24);
  const [ocrTextQuery, setOcrTextQuery] = useState("");
  const [searchMode, setSearchMode] = useState<SearchMode>("media");

  return (
    <QueryMediaForm
      file={file}
      indexError={null}
      lastIndex={indexResponse}
      limit={limit}
      ocrTextQuery={ocrTextQuery}
      onFileChange={setFile}
      onLimitChange={(value) => setLimit(Number(value || 24))}
      onOcrTextQueryChange={setOcrTextQuery}
      onSearchModeChange={setSearchMode}
      onSubmit={(event: FormEvent<HTMLFormElement>) => event.preventDefault()}
      searchError={null}
      searchMode={searchMode}
      searchPending={pending}
    />
  );
}

const meta = {
  component: QueryMediaForm,
  args: {
    file: null,
    indexError: null,
    lastIndex: null,
    limit: 24,
    ocrTextQuery: "",
    onFileChange: () => undefined,
    onLimitChange: () => undefined,
    onOcrTextQueryChange: () => undefined,
    onSearchModeChange: () => undefined,
    onSubmit: (event) => event.preventDefault(),
    searchError: null,
    searchMode: "media",
    searchPending: false,
  },
} satisfies Meta<typeof QueryMediaForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

export const SelectedMedia: Story = {
  render: () => <StatefulForm />,
};

export const Pending: Story = {
  render: () => <StatefulForm pending />,
};

export const ErrorState: Story = {
  args: {
    searchError: new Error("Search failed."),
  },
};

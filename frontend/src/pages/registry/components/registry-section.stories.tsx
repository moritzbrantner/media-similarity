import type { Meta, StoryObj } from "@storybook/react-vite";
import { Users } from "lucide-react";
import { useState } from "react";
import { inverseIndexResponse } from "../../../testing/media-fixtures";
import type { RegistryIdentity } from "../inverse-index-page";
import { RegistrySection } from "./registry-section";

function PeopleSection({ empty = false }: { empty?: boolean }) {
  const [editingIdentity, setEditingIdentity] = useState<RegistryIdentity | null>(null);
  const [mergingEntry, setMergingEntry] = useState<RegistryIdentity | null>(null);

  return (
    <RegistrySection
      emptyText="No indexed people yet."
      editingIdentity={editingIdentity}
      entries={empty ? [] : inverseIndexResponse.people}
      icon={<Users className="size-4 text-neutral-600" aria-hidden="true" />}
      kind="person"
      mergeError={null}
      mergeErrorIdentity={null}
      mergingEntry={mergingEntry}
      mergingIdentity={null}
      onMergeIdentity={async () => undefined}
      onRenameIdentity={async () => undefined}
      onSetEditingIdentity={setEditingIdentity}
      onSetMergingEntry={setMergingEntry}
      onSetSuccessText={() => undefined}
      renameError={null}
      renameErrorIdentity={null}
      renamingIdentity={null}
      title="Depicted People"
    />
  );
}

const meta = {
  component: RegistrySection,
  args: {
    emptyText: "No indexed people yet.",
    editingIdentity: null,
    entries: inverseIndexResponse.people,
    icon: <Users className="size-4 text-neutral-600" aria-hidden="true" />,
    kind: "person",
    mergeError: null,
    mergeErrorIdentity: null,
    mergingEntry: null,
    mergingIdentity: null,
    onMergeIdentity: async () => undefined,
    onRenameIdentity: async () => undefined,
    onSetEditingIdentity: () => undefined,
    onSetMergingEntry: () => undefined,
    onSetSuccessText: () => undefined,
    renameError: null,
    renameErrorIdentity: null,
    renamingIdentity: null,
    title: "Depicted People",
  },
} satisfies Meta<typeof RegistrySection>;

export default meta;

type Story = StoryObj<typeof meta>;

export const People: Story = {
  render: () => <PeopleSection />,
};

export const Empty: Story = {
  render: () => <PeopleSection empty />,
};

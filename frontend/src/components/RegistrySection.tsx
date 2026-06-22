import type { ReactNode } from "react";
import type { IdentityKind } from "../types";
import { RegistryIdentity, RegistryEntry, RegistryEntryCard } from "./inverse-index-page";

export function RegistrySection({
  emptyText, editingIdentity, entries, icon, kind, mergeError, mergeErrorIdentity, mergingEntry, mergingIdentity, onMergeIdentity, onRenameIdentity, onSetEditingIdentity, onSetMergingEntry, onSetSuccessText, renameError, renameErrorIdentity, renamingIdentity, title,
}: {
  emptyText: string;
  editingIdentity: RegistryIdentity | null;
  entries: RegistryEntry[];
  icon: ReactNode;
  kind: IdentityKind;
  mergeError: Error | null;
  mergeErrorIdentity: RegistryIdentity | null;
  mergingEntry: RegistryIdentity | null;
  mergingIdentity: RegistryIdentity | null;
  onMergeIdentity: (kind: IdentityKind, targetId: string, sourceIds: string[]) => Promise<unknown>;
  onRenameIdentity: (kind: IdentityKind, id: string, label: string) => Promise<unknown>;
  onSetEditingIdentity: (identity: RegistryIdentity | null) => void;
  onSetMergingEntry: (identity: RegistryIdentity | null) => void;
  onSetSuccessText: (text: string | null) => void;
  renameError: Error | null;
  renameErrorIdentity: RegistryIdentity | null;
  renamingIdentity: RegistryIdentity | null;
  title: string;
}) {
  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <div className="flex items-center gap-2">
        {icon}
        <h3 className="text-sm font-semibold text-neutral-950">{title}</h3>
      </div>

      {entries.length === 0 ? (
        <div className="mt-4 rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-4 py-8 text-center text-sm text-neutral-500">
          {emptyText}
        </div>
      ) : (
        <div className="mt-4 grid gap-3">
          {entries.map((entry) => (
            <RegistryEntryCard
              editingIdentity={editingIdentity}
              entries={entries}
              entry={entry}
              key={`${kind}-${entry.id}`}
              kind={kind}
              mergeError={mergeError}
              mergeErrorIdentity={mergeErrorIdentity}
              mergingEntry={mergingEntry}
              mergingIdentity={mergingIdentity}
              onMergeIdentity={onMergeIdentity}
              onRenameIdentity={onRenameIdentity}
              onSetEditingIdentity={onSetEditingIdentity}
              onSetMergingEntry={onSetMergingEntry}
              onSetSuccessText={onSetSuccessText}
              renameError={renameError}
              renameErrorIdentity={renameErrorIdentity}
              renamingIdentity={renamingIdentity} />
          ))}
        </div>
      )}
    </section>
  );
}

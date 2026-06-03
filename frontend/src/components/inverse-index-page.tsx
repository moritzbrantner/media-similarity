import { Button } from "@moritzbrantner/ui";
import {
  AlertCircle,
  Check,
  GitMerge,
  FileText,
  ImageIcon,
  Loader2,
  Mic2,
  Pencil,
  RotateCw,
  Users,
  X,
} from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import type { IdentityKind } from "../api";
import { mediaKindLabel } from "../lib/format";
import type {
  InverseIndexLocation,
  InverseIndexResponse,
  InversePersonEntry,
  InverseSpeakerEntry,
} from "../types";
import { Message } from "./status-message";

type RegistryEntry = InversePersonEntry | InverseSpeakerEntry;

type RegistryIdentity = {
  id: string;
  kind: IdentityKind;
};

export function InverseIndexPage({
  data,
  error,
  loading,
  mergeError,
  mergeErrorIdentity,
  mergingIdentity,
  onMergeIdentity,
  onRefresh,
  onRenameIdentity,
  refreshing,
  renameError,
  renameErrorIdentity,
  renamingIdentity,
}: {
  data: InverseIndexResponse | null;
  error: Error | null;
  loading: boolean;
  mergeError: Error | null;
  mergeErrorIdentity: RegistryIdentity | null;
  mergingIdentity: RegistryIdentity | null;
  onMergeIdentity: (kind: IdentityKind, targetId: string, sourceIds: string[]) => Promise<unknown>;
  onRefresh: () => void;
  onRenameIdentity: (kind: IdentityKind, id: string, label: string) => Promise<unknown>;
  refreshing: boolean;
  renameError: Error | null;
  renameErrorIdentity: RegistryIdentity | null;
  renamingIdentity: RegistryIdentity | null;
}) {
  const [editingIdentity, setEditingIdentity] = useState<RegistryIdentity | null>(null);
  const [mergingEntry, setMergingEntry] = useState<RegistryIdentity | null>(null);
  const [successText, setSuccessText] = useState<string | null>(null);

  if (loading && !data) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading inverse index" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  const people = sortPeopleEntries(data?.people ?? []);
  const speakers = sortSpeakerEntries(data?.speakers ?? []);

  return (
    <section className="flex flex-col gap-5">
      <div className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <h2 className="text-lg font-semibold text-neutral-950">Inverse Index</h2>
            <p className="mt-1 text-sm text-neutral-600">
              Key findings grouped by identity with the indexed media locations where they occur.
            </p>
          </div>
          <Button
            variant="outline"
            className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
            disabled={refreshing}
            onClick={onRefresh}
            type="button"
          >
            {refreshing ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <RotateCw className="size-4" aria-hidden="true" />
            )}
            <span>Refresh</span>
          </Button>
        </div>

        <dl className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <RegistryMetric label="Indexed media" value={data?.indexed_media ?? 0} />
          <RegistryMetric label="People" value={people.length} />
          <RegistryMetric label="Speakers" value={speakers.length} />
          <RegistryMetric label="Decode warnings" value={data?.errors.length ?? 0} />
        </dl>
      </div>

      {data?.errors.length ? (
        <Message
          icon={<AlertCircle className="size-4" />}
          text={`${data.errors.length} indexed payload(s) could not be read. The registry shows the remaining media.`}
          tone="warn"
        />
      ) : null}
      {successText ? (
        <Message icon={<Check className="size-4" />} text={successText} tone="ok" />
      ) : null}

      <div className="grid gap-5 xl:grid-cols-2">
        <RegistrySection
          emptyText="No indexed people yet. Enable face analysis, index sources, then refresh this page."
          editingIdentity={editingIdentity}
          entries={people}
          icon={<Users className="size-4 text-neutral-600" aria-hidden="true" />}
          kind="person"
          mergeError={mergeError}
          mergeErrorIdentity={mergeErrorIdentity}
          mergingEntry={mergingEntry}
          mergingIdentity={mergingIdentity}
          onMergeIdentity={onMergeIdentity}
          onSetEditingIdentity={setEditingIdentity}
          onSetMergingEntry={setMergingEntry}
          onSetSuccessText={setSuccessText}
          onRenameIdentity={onRenameIdentity}
          renameError={renameError}
          renameErrorIdentity={renameErrorIdentity}
          renamingIdentity={renamingIdentity}
          title="Depicted People"
        />
        <RegistrySection
          emptyText="No recognized speakers yet. Enable audio analysis, index audio sources, then refresh this page."
          editingIdentity={editingIdentity}
          entries={speakers}
          icon={<Mic2 className="size-4 text-neutral-600" aria-hidden="true" />}
          kind="speaker"
          mergeError={mergeError}
          mergeErrorIdentity={mergeErrorIdentity}
          mergingEntry={mergingEntry}
          mergingIdentity={mergingIdentity}
          onMergeIdentity={onMergeIdentity}
          onSetEditingIdentity={setEditingIdentity}
          onSetMergingEntry={setMergingEntry}
          onSetSuccessText={setSuccessText}
          onRenameIdentity={onRenameIdentity}
          renameError={renameError}
          renameErrorIdentity={renameErrorIdentity}
          renamingIdentity={renamingIdentity}
          title="Recognized Speakers"
        />
      </div>
    </section>
  );
}

function RegistryMetric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2">
      <dt className="text-xs font-semibold text-neutral-500">{label}</dt>
      <dd className="mt-1 text-2xl font-semibold text-neutral-950">{value}</dd>
    </div>
  );
}

function RegistrySection({
  emptyText,
  editingIdentity,
  entries,
  icon,
  kind,
  mergeError,
  mergeErrorIdentity,
  mergingEntry,
  mergingIdentity,
  onMergeIdentity,
  onRenameIdentity,
  onSetEditingIdentity,
  onSetMergingEntry,
  onSetSuccessText,
  renameError,
  renameErrorIdentity,
  renamingIdentity,
  title,
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
              renamingIdentity={renamingIdentity}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function RegistryEntryCard({
  editingIdentity,
  entries,
  entry,
  kind,
  mergeError,
  mergeErrorIdentity,
  mergingEntry,
  mergingIdentity,
  onMergeIdentity,
  onRenameIdentity,
  onSetEditingIdentity,
  onSetMergingEntry,
  onSetSuccessText,
  renameError,
  renameErrorIdentity,
  renamingIdentity,
}: {
  editingIdentity: RegistryIdentity | null;
  entries: RegistryEntry[];
  entry: RegistryEntry;
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
}) {
  const isSpeaker = kind === "speaker";
  const displayName = registryName(entry);
  const isEditing = identityMatches(editingIdentity, kind, entry.id);
  const isMerging = identityMatches(mergingEntry, kind, entry.id);
  const renamePending = identityMatches(renamingIdentity, kind, entry.id);
  const mergePending = identityMatches(mergingIdentity, kind, entry.id);
  const currentRenameError = identityMatches(renameErrorIdentity, kind, entry.id)
    ? renameError?.message
    : null;
  const currentMergeError = identityMatches(mergeErrorIdentity, kind, entry.id)
    ? mergeError?.message
    : null;
  const [labelDraft, setLabelDraft] = useState(displayName);
  const [selectedSourceIds, setSelectedSourceIds] = useState<string[]>([]);
  const mergeOptions = entries.filter((option) => option.id !== entry.id);
  const primaryCount = isSpeaker
    ? `${(entry as InverseSpeakerEntry).segment_count} segment(s)`
    : `${(entry as InversePersonEntry).face_count} face(s)`;

  useEffect(() => {
    if (isEditing) {
      setLabelDraft(displayName);
    }
  }, [displayName, isEditing]);

  useEffect(() => {
    if (!isMerging) {
      setSelectedSourceIds([]);
    }
  }, [isMerging]);

  async function saveRename() {
    try {
      await onRenameIdentity(kind, entry.id, labelDraft);
      onSetEditingIdentity(null);
      onSetSuccessText(`${displayName} renamed`);
    } catch {
      // The mutation error is rendered from React Query state.
    }
  }

  async function confirmMerge() {
    if (selectedSourceIds.length === 0) {
      return;
    }
    const message =
      kind === "person"
        ? `Merge selected people into ${displayName}? This rewrites indexed identity references.`
        : `Merge selected speakers into ${displayName}? This rewrites indexed identity references and may update the voice registry.`;
    if (!window.confirm(message)) {
      return;
    }
    try {
      await onMergeIdentity(kind, entry.id, selectedSourceIds);
      onSetMergingEntry(null);
      onSetSuccessText(`Merged ${selectedSourceIds.length} into ${displayName}`);
    } catch {
      // The mutation error is rendered from React Query state.
    }
  }

  return (
    <article className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          {isEditing ? (
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <input
                aria-label={`Label for ${displayName}`}
                className="h-9 min-w-0 flex-1 rounded-md border border-neutral-300 bg-white px-2 text-sm font-semibold text-neutral-950 shadow-sm outline-none transition focus:border-neutral-500"
                disabled={renamePending}
                onChange={(event) => setLabelDraft(event.target.value)}
                value={labelDraft}
              />
              <Button
                aria-label={`Save label for ${displayName}`}
                className="inline-flex size-9 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
                disabled={renamePending}
                onClick={saveRename}
                type="button"
                variant="outline"
              >
                {renamePending ? (
                  <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Check className="size-4" aria-hidden="true" />
                )}
              </Button>
              <Button
                aria-label={`Cancel label edit for ${displayName}`}
                className="inline-flex size-9 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
                disabled={renamePending}
                onClick={() => onSetEditingIdentity(null)}
                type="button"
                variant="outline"
              >
                <X className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ) : (
            <h4 className="truncate text-sm font-semibold text-neutral-950" title={entry.id}>
              {displayName}
            </h4>
          )}
          <p className="mt-1 truncate text-xs text-neutral-600">{entry.id}</p>
        </div>
        <div className="flex shrink-0 flex-wrap gap-2">
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {entry.media_count} media
          </span>
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {primaryCount}
          </span>
          <span className="rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
            {formatPercent(entry.confidence)}
          </span>
          <Button
            aria-label={`Rename ${displayName}`}
            className="inline-flex size-7 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
            disabled={renamePending || mergePending}
            onClick={() => {
              onSetSuccessText(null);
              onSetEditingIdentity({ id: entry.id, kind });
            }}
            title="Rename"
            type="button"
            variant="outline"
          >
            <Pencil className="size-3.5" aria-hidden="true" />
          </Button>
          <Button
            aria-label={`Merge into ${displayName}`}
            className="inline-flex size-7 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
            disabled={renamePending || mergePending || mergeOptions.length === 0}
            onClick={() => {
              onSetSuccessText(null);
              onSetMergingEntry(isMerging ? null : { id: entry.id, kind });
            }}
            title="Merge"
            type="button"
            variant="outline"
          >
            <GitMerge className="size-3.5" aria-hidden="true" />
          </Button>
        </div>
      </div>

      {currentRenameError ? (
        <p className="mt-2 text-xs font-medium text-red-700">{currentRenameError}</p>
      ) : null}

      {isSpeaker ? (
        <p className="mt-2 text-xs text-neutral-600">
          {formatDurationSeconds((entry as InverseSpeakerEntry).total_seconds)}
        </p>
      ) : null}

      {isMerging ? (
        <div className="mt-3 rounded-md border border-neutral-300 bg-white p-3">
          <div className="flex items-center justify-between gap-3">
            <p className="text-xs font-semibold text-neutral-700">Merge sources</p>
            <Button
              aria-label={`Close merge panel for ${displayName}`}
              className="inline-flex size-7 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
              onClick={() => onSetMergingEntry(null)}
              type="button"
              variant="outline"
            >
              <X className="size-3.5" aria-hidden="true" />
            </Button>
          </div>
          <div className="mt-2 grid max-h-56 gap-2 overflow-auto pr-1">
            {mergeOptions.map((option) => (
              <label
                className="grid cursor-pointer gap-2 rounded-md border border-neutral-200 bg-neutral-50 p-2 text-xs text-neutral-700 sm:grid-cols-[auto_minmax(0,1fr)]"
                key={option.id}
              >
                <input
                  checked={selectedSourceIds.includes(option.id)}
                  className="mt-1"
                  disabled={mergePending}
                  onChange={(event) => {
                    setSelectedSourceIds((current) =>
                      event.target.checked
                        ? [...current, option.id]
                        : current.filter((id) => id !== option.id),
                    );
                  }}
                  type="checkbox"
                />
                <span className="min-w-0">
                  <span className="block truncate font-semibold text-neutral-950">
                    {registryName(option)}
                  </span>
                  <span className="mt-1 block truncate text-neutral-600">{option.id}</span>
                  <span className="mt-1 block text-neutral-600">
                    {option.media_count} media - {primaryCountForEntry(kind, option)}
                  </span>
                </span>
              </label>
            ))}
          </div>
          {currentMergeError ? (
            <p className="mt-2 text-xs font-medium text-red-700">{currentMergeError}</p>
          ) : null}
          <div className="mt-3 flex justify-end">
            <Button
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
              disabled={mergePending || selectedSourceIds.length === 0}
              onClick={confirmMerge}
              type="button"
              variant="outline"
            >
              {mergePending ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <GitMerge className="size-4" aria-hidden="true" />
              )}
              <span>Confirm</span>
            </Button>
          </div>
        </div>
      ) : null}

      <div className="mt-3 grid max-h-96 gap-2 overflow-auto pr-1">
        {entry.locations.map((location) => (
          <RegistryLocationRow key={`${entry.id}-${location.media_id}`} location={location} />
        ))}
      </div>
    </article>
  );
}

function RegistryLocationRow({ location }: { location: InverseIndexLocation }) {
  const previewUrl = location.thumbnail_url;
  const openUrl = location.scene_clip_url ?? location.media_url;

  return (
    <div className="grid gap-3 rounded-md border border-neutral-200 bg-white p-2 sm:grid-cols-[64px_minmax(0,1fr)_auto]">
      <div className="grid aspect-square place-items-center overflow-hidden rounded bg-neutral-200">
        {previewUrl ? (
          <img alt="" className="h-full w-full object-cover" loading="lazy" src={previewUrl} />
        ) : (
          <ImageIcon className="size-6 text-neutral-500" aria-hidden="true" />
        )}
      </div>
      <div className="min-w-0">
        <h5 className="truncate text-sm font-semibold text-neutral-950" title={location.filename}>
          {location.filename}
        </h5>
        <p className="mt-1 truncate text-xs text-neutral-600" title={location.relative_path}>
          {location.relative_path}
        </p>
        <div className="mt-2 flex flex-wrap gap-2 text-xs">
          <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
            {mediaKindLabel(location.media_kind)}
          </span>
          <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
            {location.occurrence_count} hit(s)
          </span>
          {location.frame_indices.length ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              Frames {compactNumberList(location.frame_indices)}
            </span>
          ) : null}
          {location.start_seconds !== null && location.end_seconds !== null ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              {formatSeconds(location.start_seconds)}-{formatSeconds(location.end_seconds)}
            </span>
          ) : null}
          {location.page_number ? (
            <span className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-1 font-semibold text-neutral-700">
              Page {location.page_number}
            </span>
          ) : null}
        </div>
      </div>
      {openUrl ? (
        <a
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
          href={openUrl}
          rel="noreferrer"
          target="_blank"
        >
          <FileText className="size-4" aria-hidden="true" />
          <span>Open</span>
        </a>
      ) : null}
    </div>
  );
}

function sortPeopleEntries(people: InverseIndexResponse["people"]) {
  return [...people].sort(
    (left, right) =>
      right.media_count - left.media_count ||
      right.face_count - left.face_count ||
      registryName(left).localeCompare(registryName(right), undefined, {
        sensitivity: "base",
      }),
  );
}

function sortSpeakerEntries(speakers: InverseIndexResponse["speakers"]) {
  return [...speakers].sort(
    (left, right) =>
      right.media_count - left.media_count ||
      right.total_seconds - left.total_seconds ||
      registryName(left).localeCompare(registryName(right), undefined, {
        sensitivity: "base",
      }),
  );
}

function registryName(entry: { id: string; label: string | null }) {
  return entry.label?.trim() || entry.id;
}

function identityMatches(identity: RegistryIdentity | null, kind: IdentityKind, id: string) {
  return identity?.kind === kind && identity.id === id;
}

function primaryCountForEntry(kind: IdentityKind, entry: RegistryEntry) {
  return kind === "speaker"
    ? `${(entry as InverseSpeakerEntry).segment_count} segment(s)`
    : `${(entry as InversePersonEntry).face_count} face(s)`;
}

function compactNumberList(values: number[]) {
  if (values.length <= 4) {
    return values.join(", ");
  }

  return `${values.slice(0, 4).join(", ")} +${values.length - 4}`;
}

function formatDurationSeconds(seconds: number) {
  if (seconds < 60) {
    return `${seconds.toFixed(1)}s total`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = Math.round(seconds % 60);
  return `${minutes}m ${remainingSeconds}s total`;
}

function formatSeconds(seconds: number) {
  return `${seconds.toFixed(1)}s`;
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

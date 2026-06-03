import { Button } from "@moritzbrantner/ui";
import { Input } from "@moritzbrantner/ui";
import { Label } from "@moritzbrantner/ui";
import { NativeSelect } from "@moritzbrantner/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertCircle, FileImage, Loader2, Plus, Save, Trash2 } from "lucide-react";
import type { ComponentProps } from "react";
import { useEffect, useState } from "react";
import {
  createSmartAlbum,
  deleteIndexedMedia,
  deleteSmartAlbum,
  fetchSmartAlbumResults,
  fetchSmartAlbums,
  previewSmartAlbum,
  updateIndexedMediaTags,
  updateSmartAlbum,
} from "../api";
import { formatHistoryTime } from "../jobs/job-utils";
import type {
  AlbumSortMode,
  EditableSmartAlbum,
  SmartAlbum,
  SmartAlbumCriteria,
  SmartAlbumResultsResponse,
} from "../types";
import { ResultCard } from "./results/result-card";
import { Message } from "./status-message";

const DEFAULT_ALBUM_LIMIT = 60;

const DEFAULT_CRITERIA: SmartAlbumCriteria = {
  camera_query: null,
  captured_from: null,
  captured_to: null,
  duplicate_status: "all",
  has_gps: null,
  keyword_query: null,
  max_height: null,
  max_size_bytes: null,
  max_width: null,
  media_kind: null,
  min_height: null,
  min_size_bytes: null,
  min_width: null,
  modified_from: null,
  modified_to: null,
  name_query: null,
  orientation: null,
  person_id: null,
  source_type: null,
  speaker_id: null,
  text_query: null,
};

export function SmartAlbumsPage({
  initialDraft,
  onDraftConsumed,
}: {
  initialDraft: EditableSmartAlbum | null;
  onDraftConsumed: () => void;
}) {
  const queryClient = useQueryClient();
  const [selectedAlbumId, setSelectedAlbumId] = useState<string | null>(null);
  const [draft, setDraft] = useState<EditableSmartAlbum | null>(null);
  const [editingAlbumId, setEditingAlbumId] = useState<string | null>(null);
  const [offset, setOffset] = useState(0);

  const albumsQuery = useQuery({
    queryKey: ["smart-albums"],
    queryFn: fetchSmartAlbums,
  });
  const albums = albumsQuery.data?.albums ?? [];
  const selectedAlbum = albums.find((album) => album.id === selectedAlbumId) ?? albums[0] ?? null;
  const effectiveAlbumId = selectedAlbumId ?? selectedAlbum?.id ?? null;

  useEffect(() => {
    if (initialDraft) {
      setDraft(initialDraft);
      setEditingAlbumId(null);
      setSelectedAlbumId(null);
      setOffset(0);
      onDraftConsumed();
    }
  }, [initialDraft, onDraftConsumed]);

  useEffect(() => {
    if (!selectedAlbumId && selectedAlbum) {
      setSelectedAlbumId(selectedAlbum.id);
    }
  }, [selectedAlbum, selectedAlbumId]);

  const resultsQuery = useQuery({
    queryKey: ["smart-album-results", effectiveAlbumId, offset, selectedAlbum?.limit],
    queryFn: () => fetchSmartAlbumResults(effectiveAlbumId ?? "", offset, selectedAlbum?.limit),
    enabled: Boolean(effectiveAlbumId && !draft),
  });

  const previewMutation = useMutation({
    mutationFn: (album: EditableSmartAlbum) => previewSmartAlbum(album, offset, album.limit),
  });

  const createMutation = useMutation({
    mutationFn: createSmartAlbum,
    onSuccess: (album) => {
      setDraft(null);
      setEditingAlbumId(null);
      setSelectedAlbumId(album.id);
      setOffset(0);
      queryClient.invalidateQueries({ queryKey: ["smart-albums"] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ album, id }: { album: EditableSmartAlbum; id: string }) =>
      updateSmartAlbum(id, album),
    onSuccess: (album) => {
      setDraft(null);
      setEditingAlbumId(null);
      setSelectedAlbumId(album.id);
      setOffset(0);
      queryClient.invalidateQueries({ queryKey: ["smart-albums"] });
      queryClient.invalidateQueries({ queryKey: ["smart-album-results", album.id] });
    },
  });

  const deleteAlbumMutation = useMutation({
    mutationFn: deleteSmartAlbum,
    onSuccess: (_response, id) => {
      if (selectedAlbumId === id) {
        setSelectedAlbumId(null);
      }
      setDraft(null);
      setOffset(0);
      queryClient.invalidateQueries({ queryKey: ["smart-albums"] });
    },
  });

  const deleteMediaMutation = useMutation({
    mutationFn: deleteIndexedMedia,
    onSuccess: () => invalidateAlbumData(),
  });

  const updateTagsMutation = useMutation({
    mutationFn: updateIndexedMediaTags,
    onSuccess: () => invalidateAlbumData(),
  });

  const activeAlbum = draft ?? (selectedAlbum ? editableFromAlbum(selectedAlbum) : null);
  const response: SmartAlbumResultsResponse | null = draft
    ? (previewMutation.data ?? null)
    : (resultsQuery.data ?? null);
  const loadingResults = draft ? previewMutation.isPending : resultsQuery.isLoading;
  const error = albumsQuery.error ?? resultsQuery.error ?? previewMutation.error;
  const saving = createMutation.isPending || updateMutation.isPending;

  function invalidateAlbumData() {
    queryClient.invalidateQueries({ queryKey: ["smart-album-results"] });
    queryClient.invalidateQueries({ queryKey: ["health"] });
    queryClient.invalidateQueries({ queryKey: ["inverse-index"] });
  }

  function startNewAlbum() {
    setDraft(emptyAlbumDraft());
    setEditingAlbumId(null);
    setSelectedAlbumId(null);
    setOffset(0);
    previewMutation.reset();
  }

  function selectAlbum(album: SmartAlbum) {
    setSelectedAlbumId(album.id);
    setDraft(null);
    setEditingAlbumId(null);
    setOffset(0);
    previewMutation.reset();
  }

  function editAlbum(album: SmartAlbum) {
    setDraft(editableFromAlbum(album));
    setEditingAlbumId(album.id);
    setSelectedAlbumId(album.id);
    setOffset(0);
    previewMutation.reset();
  }

  function saveDraft() {
    if (!draft) {
      return;
    }
    if (editingAlbumId) {
      updateMutation.mutate({ album: draft, id: editingAlbumId });
      return;
    }
    createMutation.mutate(draft);
  }

  const hasNext = response ? response.offset + response.count < response.total : false;
  const hasPrevious = offset > 0;

  return (
    <section className="grid gap-5 xl:grid-cols-[320px_minmax(0,1fr)]">
      <aside className="h-fit rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-lg font-semibold text-neutral-950">Albums</h2>
          <Button
            className="inline-flex h-9 items-center gap-2 rounded-md px-3 text-sm font-semibold"
            onClick={startNewAlbum}
            type="button"
          >
            <Plus className="size-4" aria-hidden="true" />
            New
          </Button>
        </div>

        {albumsQuery.isLoading ? (
          <div className="mt-4 grid min-h-32 place-items-center text-neutral-600">
            <Loader2 className="size-6 animate-spin" aria-label="Loading albums" />
          </div>
        ) : albums.length === 0 && !draft ? (
          <div className="mt-4 rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-3 py-8 text-center text-sm text-neutral-500">
            No smart albums yet.
          </div>
        ) : (
          <div className="mt-4 flex flex-col gap-2">
            {draft && !editingAlbumId ? <DraftAlbumListItem draft={draft} /> : null}
            {albums.map((album) => (
              <button
                aria-pressed={!draft && selectedAlbumId === album.id}
                className={`rounded-md border px-3 py-2 text-left transition ${
                  !draft && selectedAlbumId === album.id
                    ? "border-emerald-700 bg-emerald-50"
                    : "border-neutral-200 bg-white hover:border-neutral-400 hover:bg-neutral-50"
                }`}
                key={album.id}
                onClick={() => selectAlbum(album)}
                type="button"
              >
                <span className="block truncate text-sm font-semibold text-neutral-950">
                  {album.name}
                </span>
                {album.description ? (
                  <span className="mt-1 block truncate text-xs text-neutral-600">
                    {album.description}
                  </span>
                ) : null}
                <span className="mt-1 block text-xs text-neutral-500">
                  Updated {formatHistoryTime(album.updated_at)}
                </span>
              </button>
            ))}
          </div>
        )}
      </aside>

      <div className="flex min-w-0 flex-col gap-5">
        {error ? (
          <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />
        ) : null}

        {activeAlbum ? (
          <AlbumEditor
            album={activeAlbum}
            canReset={Boolean(draft)}
            editing={Boolean(draft)}
            onChange={setDraft}
            onDelete={
              selectedAlbum && !draft
                ? () => {
                    if (window.confirm(`Delete smart album ${selectedAlbum.name}?`)) {
                      deleteAlbumMutation.mutate(selectedAlbum.id);
                    }
                  }
                : undefined
            }
            onEdit={selectedAlbum && !draft ? () => editAlbum(selectedAlbum) : undefined}
            onPreview={draft ? () => previewMutation.mutate(draft) : undefined}
            onReset={() => {
              setDraft(null);
              setEditingAlbumId(null);
              previewMutation.reset();
            }}
            onSave={draft ? saveDraft : undefined}
            saving={saving}
          />
        ) : (
          <div className="grid min-h-72 place-items-center rounded-lg border border-neutral-300 bg-white p-8 text-center text-sm text-neutral-500 shadow-sm">
            <div className="flex flex-col items-center gap-3">
              <FileImage className="size-8" aria-hidden="true" />
              <span>Create a smart album to browse indexed media by saved criteria.</span>
            </div>
          </div>
        )}

        <AlbumResults
          deletingId={deleteMediaMutation.isPending ? deleteMediaMutation.variables : undefined}
          loading={loadingResults}
          onDelete={(id) => deleteMediaMutation.mutate(id)}
          onNext={() => setOffset(offset + (response?.limit ?? DEFAULT_ALBUM_LIMIT))}
          onPrevious={() =>
            setOffset(Math.max(0, offset - (response?.limit ?? DEFAULT_ALBUM_LIMIT)))
          }
          onUpdateTags={(id, tags) => updateTagsMutation.mutate({ id, tags })}
          response={response}
          showNext={hasNext}
          showPrevious={hasPrevious}
          tagSavingId={updateTagsMutation.isPending ? updateTagsMutation.variables?.id : undefined}
        />
      </div>
    </section>
  );
}

function DraftAlbumListItem({ draft }: { draft: EditableSmartAlbum }) {
  return (
    <div className="rounded-md border border-dashed border-emerald-500 bg-emerald-50 px-3 py-2 text-sm font-semibold text-emerald-950">
      {draft.name.trim() || "Unsaved album"}
    </div>
  );
}

function AlbumEditor({
  album,
  canReset,
  editing,
  onChange,
  onDelete,
  onEdit,
  onPreview,
  onReset,
  onSave,
  saving,
}: {
  album: EditableSmartAlbum;
  canReset: boolean;
  editing: boolean;
  onChange: (album: EditableSmartAlbum) => void;
  onDelete?: () => void;
  onEdit?: () => void;
  onPreview?: () => void;
  onReset: () => void;
  onSave?: () => void;
  saving: boolean;
}) {
  function update<K extends keyof EditableSmartAlbum>(key: K, value: EditableSmartAlbum[K]) {
    onChange({ ...album, [key]: value });
  }
  function updateCriteria<K extends keyof SmartAlbumCriteria>(
    key: K,
    value: SmartAlbumCriteria[K],
  ) {
    update("criteria", { ...album.criteria, [key]: value });
  }

  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div className="grid flex-1 gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <FieldInput
            disabled={!editing}
            id="album-name"
            label="Name"
            onChange={(event) => update("name", event.target.value)}
            value={album.name}
          />
          <FieldInput
            disabled={!editing}
            id="album-description"
            label="Description"
            onChange={(event) => update("description", event.target.value || null)}
            value={album.description ?? ""}
          />
        </div>
        <div className="flex flex-wrap gap-2">
          {onEdit ? (
            <Button
              className="h-9 rounded-md px-3 text-sm font-semibold"
              onClick={onEdit}
              type="button"
            >
              Edit
            </Button>
          ) : null}
          {onPreview ? (
            <Button
              className="h-9 rounded-md px-3 text-sm font-semibold"
              onClick={onPreview}
              type="button"
            >
              Preview
            </Button>
          ) : null}
          {onSave ? (
            <Button
              className="inline-flex h-9 items-center gap-2 rounded-md px-3 text-sm font-semibold"
              disabled={saving}
              onClick={onSave}
              type="button"
            >
              {saving ? <Loader2 className="size-4 animate-spin" /> : <Save className="size-4" />}
              Save
            </Button>
          ) : null}
          {canReset ? (
            <Button
              className="h-9 rounded-md px-3 text-sm font-semibold"
              onClick={onReset}
              variant="outline"
              type="button"
            >
              Reset
            </Button>
          ) : null}
          {onDelete ? (
            <Button
              className="inline-flex h-9 items-center gap-2 rounded-md border-red-300 px-3 text-sm font-semibold text-red-700"
              onClick={onDelete}
              variant="outline"
              type="button"
            >
              <Trash2 className="size-4" />
              Delete
            </Button>
          ) : null}
        </div>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <FieldInput
          disabled={!editing}
          id="album-name-query"
          label="Name or path"
          onChange={(event) => updateCriteria("name_query", textOrNull(event.target.value))}
          value={album.criteria.name_query ?? ""}
        />
        <FieldInput
          disabled={!editing}
          id="album-source-type"
          label="Source type"
          onChange={(event) => updateCriteria("source_type", textOrNull(event.target.value))}
          value={album.criteria.source_type ?? ""}
        />
        <FieldInput
          disabled={!editing}
          id="album-person-id"
          label="Person ID"
          onChange={(event) => updateCriteria("person_id", textOrNull(event.target.value))}
          value={album.criteria.person_id ?? ""}
        />
        <FieldInput
          disabled={!editing}
          id="album-speaker-id"
          label="Speaker ID"
          onChange={(event) => updateCriteria("speaker_id", textOrNull(event.target.value))}
          value={album.criteria.speaker_id ?? ""}
        />
        <FieldInput
          disabled={!editing}
          id="album-text-query"
          label="Text in media"
          onChange={(event) => updateCriteria("text_query", textOrNull(event.target.value))}
          value={album.criteria.text_query ?? ""}
        />
        <FieldInput
          disabled={!editing}
          id="album-camera-query"
          label="Camera/lens"
          onChange={(event) => updateCriteria("camera_query", textOrNull(event.target.value))}
          value={album.criteria.camera_query ?? ""}
        />
        <FieldInput
          disabled={!editing}
          id="album-keyword-query"
          label="Keyword or tag"
          onChange={(event) => updateCriteria("keyword_query", textOrNull(event.target.value))}
          value={album.criteria.keyword_query ?? ""}
        />
        <FieldSelect
          disabled={!editing}
          id="album-media-kind"
          label="Media type"
          onChange={(event) =>
            updateCriteria(
              "media_kind",
              textOrNull(event.target.value) as SmartAlbumCriteria["media_kind"],
            )
          }
          value={album.criteria.media_kind ?? "all"}
        >
          <option value="all">All media</option>
          <option value="static_image">Images only</option>
          <option value="animated_gif">GIFs only</option>
          <option value="video_scene">Video scenes only</option>
          <option value="audio">Audio only</option>
          <option value="pdf_document">PDF documents only</option>
          <option value="pdf_page">PDF pages only</option>
        </FieldSelect>
        <FieldSelect
          disabled={!editing}
          id="album-has-gps"
          label="GPS metadata"
          onChange={(event) => updateCriteria("has_gps", gpsValue(event.target.value))}
          value={gpsLabel(album.criteria.has_gps)}
        >
          <option value="all">Any GPS metadata</option>
          <option value="yes">Has GPS</option>
          <option value="no">No GPS</option>
        </FieldSelect>
        <FieldSelect
          disabled={!editing}
          id="album-duplicate-status"
          label="Duplicate status"
          onChange={(event) =>
            updateCriteria(
              "duplicate_status",
              event.target.value as SmartAlbumCriteria["duplicate_status"],
            )
          }
          value={album.criteria.duplicate_status}
        >
          <option value="all">All media</option>
          <option value="only">Duplicate groups only</option>
          <option value="exclude">Exclude duplicate groups</option>
        </FieldSelect>
        <FieldSelect
          disabled={!editing}
          id="album-orientation"
          label="Orientation"
          onChange={(event) =>
            updateCriteria(
              "orientation",
              textOrNull(event.target.value) as SmartAlbumCriteria["orientation"],
            )
          }
          value={album.criteria.orientation ?? "all"}
        >
          <option value="all">Any orientation</option>
          <option value="landscape">Landscape</option>
          <option value="portrait">Portrait</option>
          <option value="square">Square</option>
        </FieldSelect>
        <FieldSelect
          disabled={!editing}
          id="album-sort"
          label="Sort"
          onChange={(event) => update("sort", event.target.value as AlbumSortMode)}
          value={album.sort}
        >
          <option value="modified_newest">Newest modified</option>
          <option value="captured_newest">Newest captured</option>
          <option value="filename">Filename</option>
          <option value="size_largest">Largest file</option>
          <option value="duplicate_group_size">Duplicate group size</option>
        </FieldSelect>
        <NumberField
          disabled={!editing}
          id="album-limit"
          label="Limit"
          max={500}
          min={1}
          onChange={(value) => update("limit", value ?? DEFAULT_ALBUM_LIMIT)}
          value={album.limit}
        />
        <DateField
          disabled={!editing}
          id="album-modified-from"
          label="Modified after"
          onChange={(value) => updateCriteria("modified_from", value)}
          value={album.criteria.modified_from}
        />
        <DateField
          disabled={!editing}
          id="album-modified-to"
          label="Modified before"
          onChange={(value) => updateCriteria("modified_to", value)}
          value={album.criteria.modified_to}
          endOfDay
        />
        <DateField
          disabled={!editing}
          id="album-captured-from"
          label="Captured after"
          onChange={(value) => updateCriteria("captured_from", value)}
          value={album.criteria.captured_from}
        />
        <DateField
          disabled={!editing}
          id="album-captured-to"
          label="Captured before"
          onChange={(value) => updateCriteria("captured_to", value)}
          value={album.criteria.captured_to}
          endOfDay
        />
        <NumberField
          disabled={!editing}
          id="album-min-width"
          label="Minimum width"
          onChange={(value) => updateCriteria("min_width", value)}
          value={album.criteria.min_width}
        />
        <NumberField
          disabled={!editing}
          id="album-max-width"
          label="Maximum width"
          onChange={(value) => updateCriteria("max_width", value)}
          value={album.criteria.max_width}
        />
        <NumberField
          disabled={!editing}
          id="album-min-height"
          label="Minimum height"
          onChange={(value) => updateCriteria("min_height", value)}
          value={album.criteria.min_height}
        />
        <NumberField
          disabled={!editing}
          id="album-max-height"
          label="Maximum height"
          onChange={(value) => updateCriteria("max_height", value)}
          value={album.criteria.max_height}
        />
        <NumberField
          disabled={!editing}
          id="album-min-size"
          label="Min file size (MB)"
          onChange={(value) =>
            updateCriteria(
              "min_size_bytes",
              value === null ? null : Math.round(value * 1024 * 1024),
            )
          }
          step="0.1"
          value={bytesToMegabytes(album.criteria.min_size_bytes)}
        />
        <NumberField
          disabled={!editing}
          id="album-max-size"
          label="Max file size (MB)"
          onChange={(value) =>
            updateCriteria(
              "max_size_bytes",
              value === null ? null : Math.round(value * 1024 * 1024),
            )
          }
          step="0.1"
          value={bytesToMegabytes(album.criteria.max_size_bytes)}
        />
      </div>
    </section>
  );
}

function AlbumResults({
  deletingId,
  loading,
  onDelete,
  onNext,
  onPrevious,
  onUpdateTags,
  response,
  showNext,
  showPrevious,
  tagSavingId,
}: {
  deletingId?: string;
  loading: boolean;
  onDelete: (id: string) => void;
  onNext: () => void;
  onPrevious: () => void;
  onUpdateTags: (id: string, tags: string[]) => void;
  response: SmartAlbumResultsResponse | null;
  showNext: boolean;
  showPrevious: boolean;
  tagSavingId?: string;
}) {
  if (loading) {
    return (
      <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white">
        <Loader2 className="size-7 animate-spin" aria-label="Loading album results" />
      </div>
    );
  }
  if (!response) {
    return null;
  }
  return (
    <section className="flex flex-col gap-3">
      {response.warnings.length ? (
        <Message
          icon={<AlertCircle className="size-4" />}
          text={response.warnings.join(" ")}
          tone="warn"
        />
      ) : null}
      <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h3 className="text-lg font-semibold text-neutral-950">Album Results</h3>
          <p className="text-sm text-neutral-600">
            {response.count} of {response.total} media, {response.duplicate_groups.length} duplicate
            group(s)
          </p>
        </div>
        <div className="flex gap-2">
          <Button disabled={!showPrevious} onClick={onPrevious} variant="outline" type="button">
            Previous
          </Button>
          <Button disabled={!showNext} onClick={onNext} variant="outline" type="button">
            Next
          </Button>
        </div>
      </div>
      {response.results.length === 0 ? (
        <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white p-8 text-center text-sm text-neutral-500">
          No indexed media matched this album.
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {response.results.map((result) => (
            <ResultCard
              deleting={deletingId === result.image.id}
              key={result.image.id}
              onDelete={onDelete}
              onUpdateTags={onUpdateTags}
              result={{
                duplicate_group_size: result.duplicate_group_size,
                image: result.image,
                query_scene_index: null,
              }}
              tagSaving={tagSavingId === result.image.id}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function FieldInput({ id, label, ...props }: ComponentProps<typeof Input> & { label: string }) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Input
        id={id}
        className="mt-1 h-9 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm"
        {...props}
      />
    </div>
  );
}

function FieldSelect({
  children,
  id,
  label,
  ...props
}: ComponentProps<typeof NativeSelect> & { label: string }) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <NativeSelect id={id} className="mt-1 w-full" {...props}>
        {children}
      </NativeSelect>
    </div>
  );
}

function NumberField({
  onChange,
  value,
  ...props
}: Omit<ComponentProps<typeof Input>, "onChange" | "value"> & {
  label: string;
  onChange: (value: number | null) => void;
  value: number | null;
}) {
  return (
    <FieldInput
      {...props}
      onChange={(event) => onChange(numberOrNull(event.target.value))}
      type="number"
      value={value ?? ""}
    />
  );
}

function DateField({
  endOfDay = false,
  onChange,
  value,
  ...props
}: Omit<ComponentProps<typeof Input>, "onChange" | "type" | "value"> & {
  endOfDay?: boolean;
  label: string;
  onChange: (value: number | null) => void;
  value: number | null;
}) {
  return (
    <FieldInput
      {...props}
      onChange={(event) => onChange(dateSeconds(event.target.value, endOfDay))}
      type="date"
      value={dateInputValue(value)}
    />
  );
}

function emptyAlbumDraft(): EditableSmartAlbum {
  return {
    criteria: DEFAULT_CRITERIA,
    description: null,
    limit: DEFAULT_ALBUM_LIMIT,
    name: "New album",
    sort: "modified_newest",
  };
}

function editableFromAlbum(album: SmartAlbum): EditableSmartAlbum {
  return {
    criteria: { ...DEFAULT_CRITERIA, ...album.criteria },
    description: album.description,
    limit: album.limit,
    name: album.name,
    sort: album.sort,
  };
}

function textOrNull(value: string) {
  const normalized = value.trim();
  return normalized || null;
}

function gpsValue(value: string) {
  if (value === "yes") return true;
  if (value === "no") return false;
  return null;
}

function gpsLabel(value: boolean | null) {
  if (value === true) return "yes";
  if (value === false) return "no";
  return "all";
}

function numberOrNull(value: string) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function bytesToMegabytes(value: number | null) {
  return value === null ? null : Math.round((value / 1024 / 1024) * 10) / 10;
}

function dateSeconds(value: string, endOfDay: boolean) {
  if (!value) return null;
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) return null;
  if (endOfDay) {
    date.setDate(date.getDate() + 1);
    date.setMilliseconds(date.getMilliseconds() - 1);
  }
  return date.getTime() / 1000;
}

function dateInputValue(value: number | null) {
  if (value === null) return "";
  return new Date(value * 1000).toISOString().slice(0, 10);
}

export function smartAlbumDraftFromSearch({
  filters,
  limit,
  ocrTextQuery,
  sortMode,
}: {
  filters: import("../search/types").MetadataFilters;
  limit: number;
  ocrTextQuery: string;
  sortMode: import("../search/types").ResultSortMode;
}): EditableSmartAlbum {
  return {
    criteria: {
      ...DEFAULT_CRITERIA,
      camera_query: textOrNull(filters.cameraQuery),
      captured_from: dateSeconds(filters.captureDateFrom, false),
      captured_to: dateSeconds(filters.captureDateTo, true),
      duplicate_status:
        filters.nearDuplicate === "only"
          ? "only"
          : filters.nearDuplicate === "exclude"
            ? "exclude"
            : "all",
      has_gps: gpsValue(filters.hasGps),
      keyword_query: textOrNull(filters.keywordQuery),
      max_height: numberOrNull(filters.maxHeight),
      max_size_bytes:
        numberOrNull(filters.maxSizeMb) === null
          ? null
          : Math.round((numberOrNull(filters.maxSizeMb) ?? 0) * 1024 * 1024),
      max_width: numberOrNull(filters.maxWidth),
      media_kind: filters.mediaKind === "all" ? null : filters.mediaKind,
      min_height: numberOrNull(filters.minHeight),
      min_size_bytes:
        numberOrNull(filters.minSizeMb) === null
          ? null
          : Math.round((numberOrNull(filters.minSizeMb) ?? 0) * 1024 * 1024),
      min_width: numberOrNull(filters.minWidth),
      modified_from: dateSeconds(filters.dateFrom, false),
      modified_to: dateSeconds(filters.dateTo, true),
      name_query: textOrNull(filters.nameQuery),
      orientation: filters.orientation === "all" ? null : filters.orientation,
      person_id: textOrNull(filters.personId),
      source_type: filters.sourceType === "all" ? null : textOrNull(filters.sourceType),
      speaker_id: null,
      text_query: textOrNull(ocrTextQuery),
    },
    description: null,
    limit,
    name: "Saved search album",
    sort: albumSortFromSearch(sortMode),
  };
}

function albumSortFromSearch(sortMode: import("../search/types").ResultSortMode): AlbumSortMode {
  if (
    sortMode === "captured_newest" ||
    sortMode === "filename" ||
    sortMode === "modified_newest" ||
    sortMode === "size_largest"
  ) {
    return sortMode;
  }
  return "modified_newest";
}

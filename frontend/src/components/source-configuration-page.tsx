import { Button, Input, Label, NativeSelect } from "@moritzbrantner/ui";
import {
  AlertCircle,
  Camera,
  CheckCircle2,
  Cloud,
  Database,
  Film,
  FolderPlus,
  HardDrive,
  Info,
  Loader2,
  Plus,
  Save,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import type {
  IndexResponse,
  ModelRuntimeStatus,
  ModelsResponse,
  SourceConfigResponse,
  SourceConfigSource,
  SupportedSourceType,
} from "../types";
import { completeIndexingConfig } from "./indexing-configuration-page";
import { Message } from "./status-message";

export type SourceDraft = {
  id: string;
  kind: string;
  spec: string;
};

function createHistoryId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function SourceConfigurationPage({
  config,
  error,
  indexError,
  indexPending,
  lastIndex,
  loading,
  modelActionPending,
  modelError,
  models,
  modelsError,
  modelsLoading,
  onDownloadModel,
  onEnableModel,
  onIndex,
  onSave,
  saveError,
  savePending,
  saveSuccess,
}: {
  config: SourceConfigResponse | null;
  error: Error | null;
  indexError: Error | null;
  indexPending: boolean;
  lastIndex: IndexResponse | null;
  loading: boolean;
  modelActionPending?: string;
  modelError: Error | null;
  models: ModelsResponse | null;
  modelsError: Error | null;
  modelsLoading: boolean;
  onDownloadModel: (role: string, model?: string | null) => void;
  onEnableModel: (role: string, model?: string | null) => void;
  onIndex: () => void;
  onSave: (sources: string[]) => void;
  saveError: Error | null;
  savePending: boolean;
  saveSuccess: boolean;
}) {
  const [drafts, setDrafts] = useState<SourceDraft[]>([]);

  useEffect(() => {
    if (!config) {
      return;
    }

    setDrafts(
      config.sources.map((source) => ({
        id: createHistoryId(),
        kind: source.kind,
        spec: source.spec,
      })),
    );
  }, [config]);

  function updateDraft(id: string, patch: Partial<SourceDraft>) {
    setDrafts((current) =>
      current.map((source) => (source.id === id ? { ...source, ...patch } : source)),
    );
  }

  function addSource(kind = "local") {
    const type =
      config?.supported_source_types.find((item) => item.kind === kind && item.implemented) ??
      config?.supported_source_types.find((item) => item.implemented);
    if (!type) {
      return;
    }
    setDrafts((current) => [
      ...current,
      {
        id: createHistoryId(),
        kind: type.kind,
        spec: type?.example.split(" or ")[0] ?? "",
      },
    ]);
  }

  function removeSource(id: string) {
    setDrafts((current) => current.filter((source) => source.id !== id));
  }

  function saveSources() {
    onSave(drafts.map((source) => source.spec.trim()).filter(Boolean));
  }

  const configuredSources = drafts.map((source) => source.spec.trim()).filter(Boolean);
  const mediaSourcesWritable = config?.media_sources_writable ?? true;
  const canSave = configuredSources.length > 0 && mediaSourcesWritable && !savePending;

  if (loading) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading source configuration" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  if (!config) {
    return null;
  }

  const indexing = completeIndexingConfig(config.indexing);

  return (
    <section className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
      <div className="flex min-w-0 flex-col gap-5">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="flex flex-col gap-3 border-b border-neutral-200 pb-4 sm:flex-row sm:items-start sm:justify-between">
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-neutral-950">Media Sources</h2>
              <p
                className="mt-1 truncate text-sm text-neutral-600"
                title={config.media_sources_file}
              >
                Stored in {config.media_sources_file}
              </p>
              {config.media_sources_seed_file ? (
                <p
                  className="mt-1 truncate text-xs text-neutral-500"
                  title={config.media_sources_seed_file}
                >
                  Seeded from {config.media_sources_seed_file}
                </p>
              ) : null}
            </div>
            <Button
              variant="outline"
              className="inline-flex h-10 shrink-0 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
              onClick={() => addSource()}
              type="button"
            >
              <Plus className="size-4" aria-hidden="true" />
              <span>Add Source</span>
            </Button>
          </div>

          <div className="mt-4 flex flex-col gap-3">
            {drafts.map((source, index) => (
              <SourceDraftRow
                index={index}
                key={source.id}
                onRemove={() => removeSource(source.id)}
                onUpdate={(patch) => updateDraft(source.id, patch)}
                source={source}
                supportedTypes={config.supported_source_types}
              />
            ))}
            {drafts.length === 0 ? (
              <div className="rounded-md border border-dashed border-neutral-300 bg-neutral-50 px-4 py-8 text-center text-sm text-neutral-500">
                No media sources configured.
              </div>
            ) : null}
          </div>

          <div className="mt-4 flex flex-col gap-3 border-t border-neutral-200 pt-4 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-h-11">
              {savePending ? (
                <Message
                  icon={<Loader2 className="size-4 animate-spin" />}
                  text="Saving source configuration."
                  tone="info"
                />
              ) : saveError ? (
                <Message
                  icon={<AlertCircle className="size-4" />}
                  text={saveError.message}
                  tone="error"
                />
              ) : saveSuccess ? (
                <Message
                  icon={<CheckCircle2 className="size-4" />}
                  text="Saved source configuration."
                  tone="ok"
                />
              ) : !mediaSourcesWritable ? (
                <Message
                  icon={<AlertCircle className="size-4" />}
                  text="Source configuration file is not writable."
                  tone="error"
                />
              ) : (
                <Message
                  icon={<Info className="size-4" />}
                  text="Index sources after changing the source list."
                  tone="info"
                />
              )}
            </div>
            <div className="flex gap-2">
              <Button
                variant="outline"
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
                disabled={indexPending}
                onClick={onIndex}
                type="button"
              >
                {indexPending ? (
                  <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Database className="size-4" aria-hidden="true" />
                )}
                <span>Index Sources</span>
              </Button>
              <Button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-emerald-700 px-4 text-sm font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={!canSave}
                onClick={saveSources}
                type="button"
              >
                {savePending ? (
                  <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Save className="size-4" aria-hidden="true" />
                )}
                <span>Save</span>
              </Button>
            </div>
          </div>
          {indexError || lastIndex ? (
            <div className="mt-3">
              {indexError ? (
                <Message
                  icon={<AlertCircle className="size-4" />}
                  text={indexError.message}
                  tone="error"
                />
              ) : lastIndex ? (
                <Message
                  icon={<CheckCircle2 className="size-4" />}
                  text={`Indexed ${lastIndex.indexed} media item(s), skipped ${lastIndex.skipped}, pruned ${lastIndex.pruned}, failed ${lastIndex.failed}.`}
                  tone={lastIndex.failed > 0 ? "warn" : "ok"}
                />
              ) : null}
            </div>
          ) : null}
        </section>

        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="mb-3 flex items-center gap-2 text-sm font-semibold text-neutral-950">
            <FolderPlus className="size-4 text-neutral-600" aria-hidden="true" />
            <span>Configured Source Status</span>
          </div>
          <div className="grid gap-3 md:grid-cols-2">
            {config.sources.map((source) => (
              <SourceStatusCard key={`${source.kind}-${source.spec}`} source={source} />
            ))}
          </div>
        </section>
      </div>

      <aside className="flex h-fit flex-col gap-5">
        <ModelStatusPanel
          actionPendingRole={modelActionPending}
          error={modelError ?? modelsError}
          loading={modelsLoading}
          models={models?.models ?? []}
          onDownload={onDownloadModel}
          onEnable={onEnableModel}
        />

        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h2 className="text-sm font-semibold text-neutral-950">Source Types</h2>
          <div className="mt-3 grid gap-2">
            {config.supported_source_types.map((sourceType) => (
              <SupportedSourceTypeRow key={sourceType.kind} sourceType={sourceType} />
            ))}
          </div>
        </section>

        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h2 className="text-sm font-semibold text-neutral-950">Indexing Behavior</h2>
          <dl className="mt-3 grid gap-2 text-sm">
            <Metric label="Collection" value={config.indexing.collection} />
            <Metric label="Images" value={indexing.image_extensions.join(", ")} />
            <Metric label="Video" value={indexing.video_extensions.join(", ")} />
            <Metric label="Audio" value={indexing.audio_extensions.join(", ")} />
            <Metric label="PDF" value={indexing.pdf_extensions.join(", ")} />
            <Metric
              label="Visual embeddings"
              value={
                indexing.visual_embedding_enabled
                  ? `${indexing.visual_embedding_model} (${indexing.visual_embedding_vector_size})`
                  : "disabled"
              }
            />
            <Metric label="Faces" value={indexing.face_analysis_enabled ? "enabled" : "disabled"} />
            <Metric
              label="Face confidence"
              value={indexing.face_detection_min_confidence.toFixed(2)}
            />
            <Metric label="Face threshold" value={indexing.face_cluster_threshold.toFixed(2)} />
            <Metric label="GIF samples" value={indexing.gif_sample_frames} />
            <Metric label="GIF motion" value={indexing.gif_motion_weight.toFixed(2)} />
            <Metric label="Video stride" value={indexing.video_frame_stride} />
            <Metric label="Video cap" value={indexing.video_max_frames ?? "none"} />
            <Metric label="PDF DPI" value={indexing.pdf_render_dpi} />
            <Metric label="PDF page cap" value={indexing.pdf_max_pages} />
            <Metric label="PDF summary pages" value={indexing.pdf_summary_pages} />
            <Metric label="OCR" value={indexing.ocr_enabled ? "enabled" : "disabled"} />
            <Metric label="OCR frames" value={indexing.ocr_max_frames} />
            <Metric
              label="Transcription"
              value={`backend-only (${indexing.audio_transcription_enabled ? "enabled" : "disabled"})`}
            />
          </dl>
        </section>
      </aside>
    </section>
  );
}

function SourceDraftRow({
  index,
  onRemove,
  onUpdate,
  source,
  supportedTypes,
}: {
  index: number;
  onRemove: () => void;
  onUpdate: (patch: Partial<SourceDraft>) => void;
  source: SourceDraft;
  supportedTypes: SupportedSourceType[];
}) {
  const inputId = `source-spec-${source.id}`;
  const selectId = `source-kind-${source.id}`;
  const selectedSourceType = supportedTypes.find((sourceType) => sourceType.kind === source.kind);
  const plannedReadOnly = selectedSourceType ? !selectedSourceType.implemented : false;
  const hasKnownType = source.kind === "custom" || selectedSourceType !== undefined;

  return (
    <div className="grid gap-3 rounded-md border border-neutral-200 bg-neutral-50 p-3 md:grid-cols-[180px_minmax(0,1fr)_40px]">
      <div>
        <Label className="text-xs font-semibold text-neutral-700" htmlFor={selectId}>
          Source {index + 1}
        </Label>
        <NativeSelect
          className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 disabled:cursor-not-allowed disabled:bg-neutral-100 disabled:text-neutral-500"
          disabled={plannedReadOnly}
          id={selectId}
          onChange={(event) => onUpdate({ kind: event.target.value })}
          value={source.kind}
        >
          {supportedTypes.map((sourceType) => (
            <option
              disabled={!sourceType.implemented}
              key={sourceType.kind}
              value={sourceType.kind}
            >
              {sourceType.label}
              {sourceType.implemented ? "" : " (planned)"}
            </option>
          ))}
          {!hasKnownType ? <option value={source.kind}>{source.kind}</option> : null}
          <option value="custom">Custom</option>
        </NativeSelect>
      </div>
      <div className="min-w-0">
        <Label className="text-xs font-semibold text-neutral-700" htmlFor={inputId}>
          Source spec
        </Label>
        <Input
          className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200 read-only:cursor-not-allowed read-only:bg-neutral-100 read-only:text-neutral-500"
          id={inputId}
          onChange={(event) => onUpdate({ spec: event.target.value })}
          placeholder="/images or minio://bucket/prefix"
          readOnly={plannedReadOnly}
          value={source.spec}
        />
      </div>
      <div className="flex items-end">
        <Button
          aria-label={`Remove source ${index + 1}`}
          variant="outline"
          className="inline-flex h-10 w-10 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-red-300 hover:bg-red-50 hover:text-red-700"
          onClick={onRemove}
          title="Remove source"
          type="button"
        >
          <Trash2 className="size-4" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}

function SourceStatusCard({ source }: { source: SourceConfigSource }) {
  const Icon = sourceKindIcon(source.kind);
  const toneClass =
    {
      not_implemented: "border-amber-200 bg-amber-50 text-amber-900",
      ready: "border-emerald-200 bg-emerald-50 text-emerald-900",
      unavailable: "border-red-200 bg-red-50 text-red-900",
      unsupported: "border-red-200 bg-red-50 text-red-900",
    }[source.status] ?? "border-neutral-200 bg-neutral-50 text-neutral-800";

  return (
    <article className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-neutral-600" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold text-neutral-950" title={source.spec}>
            {source.spec}
          </h3>
          <div className="mt-2 flex flex-wrap gap-2">
            <span className="inline-flex rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-700">
              {source.kind}
            </span>
            <span
              className={`inline-flex rounded-md border px-2 py-1 text-xs font-semibold ${toneClass}`}
            >
              {source.status.replaceAll("_", " ")}
            </span>
          </div>
          {source.detail ? <p className="mt-2 text-xs text-neutral-600">{source.detail}</p> : null}
        </div>
      </div>
    </article>
  );
}

function ModelStatusPanel({
  actionPendingRole,
  error,
  loading,
  models,
  onDownload,
  onEnable,
}: {
  actionPendingRole?: string;
  error: Error | null;
  loading: boolean;
  models: ModelRuntimeStatus[];
  onDownload: (role: string, model?: string | null) => void;
  onEnable: (role: string, model?: string | null) => void;
}) {
  return (
    <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
      <h2 className="text-sm font-semibold text-neutral-950">Model Status</h2>
      {loading ? (
        <div className="mt-3 flex items-center gap-2 text-sm text-neutral-600">
          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          <span>Checking models.</span>
        </div>
      ) : error ? (
        <div className="mt-3">
          <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />
        </div>
      ) : (
        <div className="mt-3 grid gap-3">
          {models.map((model) => {
            const pending = actionPendingRole === model.role;
            return (
              <article
                className="rounded-md border border-neutral-200 bg-neutral-50 p-3"
                key={model.role}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="text-sm font-semibold text-neutral-950">{model.label}</h3>
                    <p className="mt-1 truncate text-xs text-neutral-600" title={model.configured}>
                      {model.configured}
                    </p>
                  </div>
                  <span
                    className={`shrink-0 rounded-md border px-2 py-1 text-xs font-semibold ${
                      model.active
                        ? "border-emerald-200 bg-emerald-50 text-emerald-800"
                        : model.cached
                          ? "border-sky-200 bg-sky-50 text-sky-800"
                          : "border-amber-200 bg-amber-50 text-amber-800"
                    }`}
                  >
                    {model.active ? "active" : model.cached ? "cached" : "missing"}
                  </span>
                </div>
                {model.detail ? (
                  <p className="mt-2 text-xs text-neutral-600">{model.detail}</p>
                ) : null}
                <div className="mt-3 flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
                    disabled={pending || model.cached}
                    onClick={() => onDownload(model.role, model.configured)}
                    type="button"
                  >
                    {pending && !model.cached ? (
                      <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <Cloud className="size-3.5" aria-hidden="true" />
                    )}
                    <span>Download</span>
                  </Button>
                  <Button
                    variant="outline"
                    className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={pending || !model.cached}
                    onClick={() => onEnable(model.role, model.configured)}
                    type="button"
                  >
                    {pending && model.cached ? (
                      <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <CheckCircle2 className="size-3.5" aria-hidden="true" />
                    )}
                    <span>Enable</span>
                  </Button>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}

function SupportedSourceTypeRow({ sourceType }: { sourceType: SupportedSourceType }) {
  const Icon = sourceKindIcon(sourceType.kind);

  return (
    <div className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
      <div className="flex items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-neutral-600" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-sm font-semibold text-neutral-950">{sourceType.label}</h3>
            <span
              className={`shrink-0 rounded-md border px-2 py-1 text-xs font-semibold ${
                sourceType.implemented
                  ? "border-emerald-200 bg-emerald-50 text-emerald-800"
                  : "border-amber-200 bg-amber-50 text-amber-800"
              }`}
            >
              {sourceType.implemented ? "available" : "planned"}
            </span>
          </div>
          <p className="mt-1 truncate text-xs text-neutral-600" title={sourceType.example}>
            {sourceType.example}
          </p>
        </div>
      </div>
    </div>
  );
}

function sourceKindIcon(kind: string) {
  switch (kind) {
    case "camera":
      return Camera;
    case "minio":
    case "s3":
      return Cloud;
    case "video":
      return Film;
    case "local":
      return HardDrive;
    default:
      return FolderPlus;
  }
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-neutral-600">{label}</dt>
      <dd className="min-w-0 truncate font-medium text-neutral-900">{value}</dd>
    </div>
  );
}

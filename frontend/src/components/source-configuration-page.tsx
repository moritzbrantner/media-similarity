import { Button } from "@moritzbrantner/ui";
import {
  AlertCircle,
  CheckCircle2,
  Database,
  FolderPlus,
  Info,
  Loader2,
  Plus,
  Save,
} from "lucide-react";
import { useEffect, useState } from "react";
import { formatIndexSummary } from "../indexing/summary";
import type { IndexResponse, ModelsResponse, SourceConfigResponse } from "../types";
import { completeIndexingConfig } from "./indexing-configuration-page";
import { Metric } from "./source-config/metric";
import { ModelStatusPanel } from "./source-config/model-status-panel";
import { SourceDraftRow } from "./source-config/source-draft-row";
import type { SourceDraft } from "./source-config/source-draft";
import { SourceStatusCard } from "./source-config/source-status-card";
import { SupportedSourceTypeRow } from "./source-config/supported-source-type-row";
import { Message } from "./status-message";

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
  onDownloadAllModels,
  onDownloadModel,
  onDisableModel,
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
  onDownloadAllModels: () => void;
  onDownloadModel: (role: string, model?: string | null) => void;
  onDisableModel: (role: string) => void;
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
                  text={formatIndexSummary(lastIndex)}
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
          onDownloadAll={onDownloadAllModels}
          onDownload={onDownloadModel}
          onDisable={onDisableModel}
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

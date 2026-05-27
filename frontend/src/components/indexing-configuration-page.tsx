import { Button } from "@moritzbrantner/ui/components/button";
import { Checkbox } from "@moritzbrantner/ui/components/checkbox";
import { Input } from "@moritzbrantner/ui/components/input";
import { Label } from "@moritzbrantner/ui/components/label";
import { Textarea } from "@moritzbrantner/ui/components/textarea";
import {
  AlertCircle,
  CheckCircle2,
  Database,
  FileImage,
  FileText,
  Film,
  Info,
  Loader2,
  Save,
  SlidersHorizontal,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { IndexResponse, SourceConfigResponse, SourceIndexingConfig } from "../types";
import { Message } from "./status-message";

export function completeIndexingConfig(indexing: SourceIndexingConfig): SourceIndexingConfig {
  return {
    ...indexing,
    audio_extensions: indexing.audio_extensions ?? [".mp3", ".wav"],
    audio_transcription_enabled: indexing.audio_transcription_enabled ?? false,
    collection: indexing.collection ?? "",
    face_analysis_enabled: indexing.face_analysis_enabled ?? false,
    face_cluster_threshold: indexing.face_cluster_threshold ?? 0.38,
    face_detection_min_confidence: indexing.face_detection_min_confidence ?? 0.75,
    face_max_frames_per_media: indexing.face_max_frames_per_media ?? 8,
    face_min_cluster_images: indexing.face_min_cluster_images ?? 2,
    gif_default_frame_delay_ms: indexing.gif_default_frame_delay_ms ?? 100,
    gif_max_decode_frames: indexing.gif_max_decode_frames ?? 512,
    gif_motion_weight: indexing.gif_motion_weight ?? 0.2,
    gif_preview_frames: indexing.gif_preview_frames ?? 16,
    gif_sample_frames: indexing.gif_sample_frames ?? 16,
    image_extensions: indexing.image_extensions ?? [".jpg", ".jpeg", ".png", ".gif"],
    ocr_enabled: indexing.ocr_enabled ?? false,
    ocr_max_frames: indexing.ocr_max_frames ?? 4,
    pdf_extensions: indexing.pdf_extensions ?? [".pdf"],
    pdf_max_pages: indexing.pdf_max_pages ?? 100,
    pdf_render_dpi: indexing.pdf_render_dpi ?? 144,
    pdf_summary_pages: indexing.pdf_summary_pages ?? 8,
    video_extensions: indexing.video_extensions ?? [".mp4", ".mov"],
    video_frame_stride: indexing.video_frame_stride ?? 30,
    video_max_frames: indexing.video_max_frames ?? null,
    visual_embedding_enabled: indexing.visual_embedding_enabled ?? false,
    visual_embedding_model: indexing.visual_embedding_model ?? "",
    visual_embedding_vector_size: indexing.visual_embedding_vector_size ?? 0,
  };
}

function splitExtensionDraft(value: string) {
  return value
    .split(/[\s,]+/)
    .map((extension) => extension.trim())
    .filter(Boolean);
}

function normalizeExtensionDraft(value: string[]) {
  return Array.from(
    new Set(
      value
        .map((extension) => extension.trim().toLowerCase())
        .filter(Boolean)
        .map((extension) => (extension.startsWith(".") ? extension : `.${extension}`)),
    ),
  );
}

function numberInputValue(value: string, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function IndexingConfigurationPage({
  config,
  error,
  indexError,
  indexPending,
  lastIndex,
  loading,
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
  onIndex: () => void;
  onSave: (indexing: SourceIndexingConfig) => void;
  saveError: Error | null;
  savePending: boolean;
  saveSuccess: boolean;
}) {
  const [draft, setDraft] = useState<SourceIndexingConfig | null>(null);

  useEffect(() => {
    if (!config) {
      return;
    }
    setDraft(completeIndexingConfig(config.indexing));
  }, [config]);

  function updateDraft<Key extends keyof SourceIndexingConfig>(
    key: Key,
    value: SourceIndexingConfig[Key],
  ) {
    setDraft((current) => (current ? { ...current, [key]: value } : current));
  }

  function saveDraft() {
    if (!draft) {
      return;
    }
    onSave({
      ...draft,
      audio_extensions: normalizeExtensionDraft(draft.audio_extensions),
      image_extensions: normalizeExtensionDraft(draft.image_extensions),
      pdf_extensions: normalizeExtensionDraft(draft.pdf_extensions),
    });
  }

  if (loading) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading indexing configuration" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  if (!draft) {
    return null;
  }

  const canSave =
    !savePending &&
    normalizeExtensionDraft(draft.image_extensions).length > 0 &&
    normalizeExtensionDraft(draft.audio_extensions).length > 0 &&
    normalizeExtensionDraft(draft.pdf_extensions).length > 0;

  return (
    <section className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
      <div className="flex min-w-0 flex-col gap-5">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <div className="flex flex-col gap-3 border-b border-neutral-200 pb-4 sm:flex-row sm:items-start sm:justify-between">
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-neutral-950">Indexing Configuration</h2>
              <p className="mt-1 text-sm text-neutral-600">
                Applies to future indexing jobs in this running service.
              </p>
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
                onClick={saveDraft}
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

          <div className="mt-4 grid gap-5">
            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <FileImage className="size-4 text-neutral-600" aria-hidden="true" />
                <span>File Types</span>
              </h3>
              <div className="mt-3 grid gap-3 lg:grid-cols-3">
                <ExtensionsField
                  id="image-extensions"
                  label="Image extensions"
                  onChange={(value) => updateDraft("image_extensions", value)}
                  value={draft.image_extensions}
                />
                <ExtensionsField
                  id="audio-extensions"
                  label="Audio extensions"
                  onChange={(value) => updateDraft("audio_extensions", value)}
                  value={draft.audio_extensions}
                />
                <ExtensionsField
                  id="pdf-extensions"
                  label="PDF extensions"
                  onChange={(value) => updateDraft("pdf_extensions", value)}
                  value={draft.pdf_extensions}
                />
              </div>
            </section>

            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <SlidersHorizontal className="size-4 text-neutral-600" aria-hidden="true" />
                <span>Analysis</span>
              </h3>
              <div className="mt-3 grid gap-3 md:grid-cols-3">
                <ToggleField
                  checked={draft.face_analysis_enabled}
                  label="Face analysis"
                  onChange={(value) => updateDraft("face_analysis_enabled", value)}
                />
                <ToggleField
                  checked={draft.ocr_enabled}
                  label="OCR"
                  onChange={(value) => updateDraft("ocr_enabled", value)}
                />
                <ToggleField
                  checked={draft.audio_transcription_enabled}
                  label="Audio transcription"
                  onChange={(value) => updateDraft("audio_transcription_enabled", value)}
                />
              </div>
            </section>

            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <Film className="size-4 text-neutral-600" aria-hidden="true" />
                <span>Video and GIF</span>
              </h3>
              <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                <NumberField
                  id="video-frame-stride"
                  label="Video frame stride"
                  min={1}
                  onChange={(value) => updateDraft("video_frame_stride", value)}
                  value={draft.video_frame_stride}
                />
                <OptionalNumberField
                  id="video-max-frames"
                  label="Video max frames"
                  min={1}
                  onChange={(value) => updateDraft("video_max_frames", value)}
                  value={draft.video_max_frames}
                />
                <NumberField
                  id="gif-sample-frames"
                  label="GIF sample frames"
                  min={1}
                  onChange={(value) => updateDraft("gif_sample_frames", value)}
                  value={draft.gif_sample_frames}
                />
                <NumberField
                  id="gif-max-decode-frames"
                  label="GIF decode cap"
                  min={1}
                  onChange={(value) => updateDraft("gif_max_decode_frames", value)}
                  value={draft.gif_max_decode_frames}
                />
                <NumberField
                  id="gif-preview-frames"
                  label="GIF preview frames"
                  min={1}
                  onChange={(value) => updateDraft("gif_preview_frames", value)}
                  value={draft.gif_preview_frames}
                />
                <NumberField
                  id="gif-default-frame-delay"
                  label="GIF frame delay (ms)"
                  min={1}
                  onChange={(value) => updateDraft("gif_default_frame_delay_ms", value)}
                  value={draft.gif_default_frame_delay_ms}
                />
                <NumberField
                  id="gif-motion-weight"
                  label="GIF motion weight"
                  max={1}
                  min={0}
                  onChange={(value) => updateDraft("gif_motion_weight", value)}
                  step={0.01}
                  value={draft.gif_motion_weight}
                />
              </div>
            </section>

            <section>
              <h3 className="flex items-center gap-2 text-sm font-semibold text-neutral-950">
                <FileText className="size-4 text-neutral-600" aria-hidden="true" />
                <span>PDF, OCR, and Faces</span>
              </h3>
              <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                <NumberField
                  id="pdf-render-dpi"
                  label="PDF render DPI"
                  max={300}
                  min={72}
                  onChange={(value) => updateDraft("pdf_render_dpi", value)}
                  value={draft.pdf_render_dpi}
                />
                <NumberField
                  id="pdf-max-pages"
                  label="PDF page cap"
                  min={1}
                  onChange={(value) => updateDraft("pdf_max_pages", value)}
                  value={draft.pdf_max_pages}
                />
                <NumberField
                  id="pdf-summary-pages"
                  label="PDF summary pages"
                  max={256}
                  min={1}
                  onChange={(value) => updateDraft("pdf_summary_pages", value)}
                  value={draft.pdf_summary_pages}
                />
                <NumberField
                  id="ocr-max-frames"
                  label="OCR frames"
                  max={64}
                  min={1}
                  onChange={(value) => updateDraft("ocr_max_frames", value)}
                  value={draft.ocr_max_frames}
                />
                <NumberField
                  id="face-confidence"
                  label="Face confidence"
                  max={1}
                  min={0}
                  onChange={(value) => updateDraft("face_detection_min_confidence", value)}
                  step={0.01}
                  value={draft.face_detection_min_confidence}
                />
                <NumberField
                  id="face-threshold"
                  label="Face threshold"
                  max={2}
                  min={0}
                  onChange={(value) => updateDraft("face_cluster_threshold", value)}
                  step={0.01}
                  value={draft.face_cluster_threshold}
                />
                <NumberField
                  id="face-min-cluster-images"
                  label="Face cluster images"
                  min={1}
                  onChange={(value) => updateDraft("face_min_cluster_images", value)}
                  value={draft.face_min_cluster_images}
                />
                <NumberField
                  id="face-max-frames"
                  label="Face frames per media"
                  min={1}
                  onChange={(value) => updateDraft("face_max_frames_per_media", value)}
                  value={draft.face_max_frames_per_media}
                />
              </div>
            </section>
          </div>

          <div className="mt-4 border-t border-neutral-200 pt-4">
            {savePending ? (
              <Message
                icon={<Loader2 className="size-4 animate-spin" />}
                text="Saving indexing configuration."
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
                text="Saved indexing configuration."
                tone="ok"
              />
            ) : (
              <Message
                icon={<Info className="size-4" />}
                text="Run indexing after changing analysis settings."
                tone="info"
              />
            )}
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
          </div>
        </section>
      </div>

      <aside className="flex h-fit flex-col gap-5">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h2 className="text-sm font-semibold text-neutral-950">Runtime Bound</h2>
          <dl className="mt-3 grid gap-2 text-sm">
            <Metric label="Collection" value={draft.collection} />
            <Metric label="Video extensions" value={draft.video_extensions.join(", ")} />
            <Metric
              label="Visual embeddings"
              value={
                draft.visual_embedding_enabled
                  ? `${draft.visual_embedding_model} (${draft.visual_embedding_vector_size})`
                  : "disabled"
              }
            />
          </dl>
        </section>
      </aside>
    </section>
  );
}

function ExtensionsField({
  id,
  label,
  onChange,
  value,
}: {
  id: string;
  label: string;
  onChange: (value: string[]) => void;
  value: string[];
}) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Textarea
        className="mt-1 min-h-24 w-full resize-y rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
        id={id}
        onChange={(event) => onChange(splitExtensionDraft(event.target.value))}
        spellCheck={false}
        value={value.join(", ")}
      />
    </div>
  );
}

function ToggleField({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <Label className="flex h-12 items-center justify-between gap-3 rounded-md border border-neutral-200 bg-neutral-50 px-3 text-sm font-semibold text-neutral-800">
      <span>{label}</span>
      <Checkbox
        checked={checked}
        className="size-4 accent-emerald-700"
        onCheckedChange={(value) => onChange(value === true)}
      />
    </Label>
  );
}

function NumberField({
  id,
  label,
  max,
  min,
  onChange,
  step = 1,
  value,
}: {
  id: string;
  label: string;
  max?: number;
  min: number;
  onChange: (value: number) => void;
  step?: number;
  value: number;
}) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Input
        className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
        id={id}
        max={max}
        min={min}
        onChange={(event) => onChange(numberInputValue(event.target.value, min))}
        step={step}
        type="number"
        value={value}
      />
    </div>
  );
}

function OptionalNumberField({
  id,
  label,
  min,
  onChange,
  value,
}: {
  id: string;
  label: string;
  min: number;
  onChange: (value: number | null) => void;
  value: number | null;
}) {
  return (
    <div>
      <Label className="text-xs font-semibold text-neutral-700" htmlFor={id}>
        {label}
      </Label>
      <Input
        className="mt-1 h-10 w-full rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 outline-none transition focus:border-emerald-700 focus:ring-2 focus:ring-emerald-200"
        id={id}
        min={min}
        onChange={(event) =>
          onChange(event.target.value === "" ? null : numberInputValue(event.target.value, min))
        }
        placeholder="No cap"
        type="number"
        value={value ?? ""}
      />
    </div>
  );
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-neutral-600">{label}</dt>
      <dd className="min-w-0 truncate font-medium text-neutral-900">{value}</dd>
    </div>
  );
}

import { Badge, Button, Input, Label } from "@moritzbrantner/ui";
import {
  FileAudio,
  FileImage,
  FileText,
  FileVideo,
  ImageIcon,
  Loader2,
  Save,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { formatFileSize, formatModifiedAt } from "../lib/format";
import { captureTimeMs } from "../search/filtering";
import type { PersonSummary, SearchResult } from "../types";

export function ResultsGrid({
  deletingId,
  onDelete,
  onUpdateTags,
  pending,
  results,
  searched,
  tagSavingId,
}: {
  deletingId?: string;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  pending: boolean;
  results: SearchResult[];
  searched: boolean;
  tagSavingId?: string;
}) {
  if (pending) {
    return (
      <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600">
        <Loader2 className="size-7 animate-spin" aria-label="Loading search results" />
      </div>
    );
  }

  if (!searched) {
    return <EmptyResults text="Choose a query image, video, audio, or PDF and run a search." />;
  }

  if (results.length === 0) {
    return <EmptyResults text="No indexed media matched this query." />;
  }

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {results.map((result) => (
        <ResultCard
          deleting={deletingId === result.image.id}
          key={result.image.id}
          onDelete={onDelete}
          onUpdateTags={onUpdateTags}
          result={result}
          tagSaving={tagSavingId === result.image.id}
        />
      ))}
    </div>
  );
}

function VideoSceneLinks({ image }: { image: SearchResult["image"] }) {
  if (image.media_kind !== "video_scene") {
    return null;
  }

  const start = image.scene_start_seconds ?? 0;
  const end = image.scene_end_seconds ?? start;
  const fullVideoUrl = image.full_video_url
    ? `${image.full_video_url}#t=${start.toFixed(3)},${end.toFixed(3)}`
    : null;

  return (
    <div className="grid gap-2">
      <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
        {formatSeconds(start)}-{formatSeconds(end)}
        {image.scene_start_frame !== null && image.scene_end_frame !== null
          ? ` · frames ${image.scene_start_frame}-${image.scene_end_frame}`
          : ""}
      </div>
      <div className="flex flex-wrap gap-2">
        {fullVideoUrl ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={fullVideoUrl}
            rel="noreferrer"
            target="_blank"
          >
            <FileVideo className="size-3.5" aria-hidden="true" />
            <span>Full video</span>
          </a>
        ) : null}
        {image.scene_clip_url ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={image.scene_clip_url}
            rel="noreferrer"
            target="_blank"
          >
            <FileVideo className="size-3.5" aria-hidden="true" />
            <span>Scene clip</span>
          </a>
        ) : null}
      </div>
    </div>
  );
}

function PhotoMetadataDetails({
  metadata,
}: {
  metadata: NonNullable<SearchResult["image"]["photo_metadata"]>;
}) {
  return (
    <details className="rounded-md border border-neutral-200 bg-neutral-50 p-3 text-sm">
      <summary className="cursor-pointer font-semibold text-neutral-800">Photo metadata</summary>
      <dl className="mt-3 grid gap-2">
        {metadata.raw.map((entry, index) => (
          <div className="grid gap-1" key={`${entry.namespace}-${entry.key}-${index}`}>
            <dt className="text-xs font-semibold uppercase text-neutral-500">
              {entry.namespace} · {entry.label || entry.key}
            </dt>
            <dd className="break-words text-neutral-900">{entry.value}</dd>
          </div>
        ))}
      </dl>
    </details>
  );
}

function AudioLinks({ image }: { image: SearchResult["image"] }) {
  if (image.media_kind !== "audio" || !image.full_audio_url) {
    return null;
  }

  const start = image.scene_start_seconds ?? null;
  const end = image.scene_end_seconds ?? null;
  const audioUrl =
    start !== null && end !== null
      ? `${image.full_audio_url}#t=${start.toFixed(3)},${end.toFixed(3)}`
      : image.full_audio_url;

  return (
    <div className="grid gap-2">
      <audio className="w-full" controls src={audioUrl} />
      {start !== null && end !== null ? (
        <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
          {formatSeconds(start)}-{formatSeconds(end)}
        </div>
      ) : null}
      <a
        className="inline-flex h-8 w-fit items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
        href={audioUrl}
        rel="noreferrer"
        target="_blank"
      >
        <FileAudio className="size-3.5" aria-hidden="true" />
        <span>Open audio</span>
      </a>
    </div>
  );
}

function PdfLinks({ image }: { image: SearchResult["image"] }) {
  if (image.media_kind !== "pdf_page" && image.media_kind !== "pdf_document") {
    return null;
  }

  const pageUrl =
    image.pdf_page_url ??
    (image.full_pdf_url && image.pdf_page_number
      ? `${image.full_pdf_url}#page=${image.pdf_page_number}`
      : null);

  return (
    <div className="grid gap-2">
      {image.pdf_page_number ? (
        <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
          Page {image.pdf_page_number}
          {image.pdf_page_count ? ` of ${image.pdf_page_count}` : ""}
        </div>
      ) : image.pdf_page_count ? (
        <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-xs text-neutral-700">
          {image.pdf_page_count} page(s)
        </div>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {image.full_pdf_url ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={image.full_pdf_url}
            rel="noreferrer"
            target="_blank"
          >
            <FileText className="size-3.5" aria-hidden="true" />
            <span>Open PDF</span>
          </a>
        ) : null}
        {pageUrl ? (
          <a
            className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50"
            href={pageUrl}
            rel="noreferrer"
            target="_blank"
          >
            <FileText className="size-3.5" aria-hidden="true" />
            <span>Open page</span>
          </a>
        ) : null}
      </div>
    </div>
  );
}

function EmptyResults({ text }: { text: string }) {
  return (
    <div className="grid min-h-44 place-items-center rounded-lg border border-neutral-300 bg-white p-8 text-center text-sm text-neutral-500">
      <div className="flex flex-col items-center gap-3">
        <FileImage className="size-8" aria-hidden="true" />
        <span>{text}</span>
      </div>
    </div>
  );
}

function ResultCard({
  deleting = false,
  onDelete,
  onUpdateTags,
  result,
  tagSaving = false,
}: {
  deleting?: boolean;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  result: SearchResult;
  tagSaving?: boolean;
}) {
  const image = result.image;
  const faces = image.faces ?? [];
  const people = image.people ?? [];
  const photoMetadata = image.photo_metadata;
  const cameraLabel = photoMetadata ? photoCameraLabel(photoMetadata) : null;
  const previewUrl = image.animated_thumbnail_url ?? image.thumbnail_url;

  return (
    <article className="overflow-hidden rounded-lg border border-neutral-300 bg-white shadow-sm">
      <div className="grid aspect-[4/3] place-items-center bg-neutral-200">
        {previewUrl ? (
          <img alt="" className="h-full w-full object-contain" loading="lazy" src={previewUrl} />
        ) : (
          <ImageIcon className="size-9 text-neutral-500" aria-hidden="true" />
        )}
      </div>
      <div className="flex flex-col gap-3 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold text-neutral-950" title={image.filename}>
              {image.filename}
            </h3>
            <p className="mt-1 truncate text-xs text-neutral-600" title={image.relative_path}>
              {image.relative_path}
            </p>
          </div>
          {onDelete ? (
            <Button
              aria-label={`Delete ${image.filename} from index`}
              variant="outline"
              className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-neutral-300 bg-white text-neutral-700 transition hover:border-red-300 hover:bg-red-50 hover:text-red-700 disabled:cursor-wait disabled:opacity-60"
              disabled={deleting}
              onClick={() => {
                if (window.confirm(`Delete ${image.filename} from the index?`)) {
                  onDelete(image.id);
                }
              }}
              title="Delete from index"
              type="button"
            >
              {deleting ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Trash2 className="size-4" aria-hidden="true" />
              )}
            </Button>
          ) : null}
        </div>

        <dl className="grid gap-2 text-sm">
          <Metric label="Visual score" value={result.vector_score.toFixed(4)} />
          <Metric label="pHash distance" value={result.hash_distance ?? "n/a"} />
          {result.ocr_score !== null && result.ocr_score !== undefined ? (
            <Metric label="OCR score" value={result.ocr_score.toFixed(2)} />
          ) : null}
          <Metric label="Dimensions" value={`${image.width} x ${image.height}`} />
          <Metric label="File size" value={formatFileSize(image.size_bytes)} />
          <Metric label="Modified" value={formatModifiedAt(image.modified_at)} />
          {image.frame_count ? <Metric label="Frames" value={image.frame_count} /> : null}
          {image.duration_ms ? (
            <Metric label="Duration" value={formatDuration(image.duration_ms)} />
          ) : null}
          {image.pdf_page_number ? (
            <Metric
              label="PDF page"
              value={
                image.pdf_page_count
                  ? `${image.pdf_page_number} of ${image.pdf_page_count}`
                  : image.pdf_page_number
              }
            />
          ) : image.pdf_page_count ? (
            <Metric label="PDF pages" value={image.pdf_page_count} />
          ) : null}
          {image.audio_analysis ? (
            <Metric
              label="Speech"
              value={image.audio_analysis.speech_detected ? "Detected" : "Not detected"}
            />
          ) : null}
          {image.audio_analysis ? (
            <Metric label="Speech ratio" value={formatPercent(image.audio_analysis.speech_ratio)} />
          ) : null}
          {image.audio_analysis?.audio_segments?.length ? (
            <Metric label="Audio bits" value={image.audio_analysis.audio_segments.length} />
          ) : null}
          {image.audio_analysis?.recognized_voices?.length ? (
            <Metric
              label="Voices"
              value={image.audio_analysis.recognized_voices.map((voice) => voice.label).join(", ")}
            />
          ) : null}
          {image.audio_analysis?.transcript_text ? (
            <Metric label="Transcript" value={image.audio_analysis.transcript_text} />
          ) : null}
          {image.audio_analysis?.transcript_language ? (
            <Metric label="Language" value={image.audio_analysis.transcript_language} />
          ) : null}
          {image.audio_analysis?.tempo_bpm ? (
            <Metric label="Tempo" value={`${image.audio_analysis.tempo_bpm.toFixed(1)} BPM`} />
          ) : null}
          {image.audio_analysis?.tempo_bpm ? (
            <Metric
              label="Tempo confidence"
              value={formatPercent(image.audio_analysis.tempo_confidence)}
            />
          ) : null}
          {photoMetadata?.capture_time ? (
            <Metric label="Captured" value={formatCaptureTime(photoMetadata.capture_time)} />
          ) : null}
          {cameraLabel ? <Metric label="Camera" value={cameraLabel} /> : null}
          {photoMetadata?.lens_model ? (
            <Metric label="Lens" value={photoMetadata.lens_model} />
          ) : null}
          {photoMetadata?.gps ? <Metric label="GPS" value={formatGps(photoMetadata.gps)} /> : null}
          {photoMetadata?.rating !== null && photoMetadata?.rating !== undefined ? (
            <Metric label="Rating" value={photoMetadata.rating} />
          ) : null}
          {photoMetadata?.keywords?.length ? (
            <Metric label="Keywords" value={photoMetadata.keywords.join(", ")} />
          ) : null}
          {photoMetadata?.creator ? <Metric label="Creator" value={photoMetadata.creator} /> : null}
          {photoMetadata?.copyright ? (
            <Metric label="Copyright" value={photoMetadata.copyright} />
          ) : null}
          {image.ocr_text ? <Metric label="OCR text" value={image.ocr_text} /> : null}
          {faces.length ? <Metric label="Faces" value={faces.length} /> : null}
          {people.length ? (
            <Metric label="People" value={people.map(personDisplayName).join(", ")} />
          ) : null}
        </dl>

        <MediaTagEditor image={image} onUpdateTags={onUpdateTags} saving={tagSaving} />

        {photoMetadata?.raw?.length ? <PhotoMetadataDetails metadata={photoMetadata} /> : null}

        <VideoSceneLinks image={image} />
        <AudioLinks image={image} />
        <PdfLinks image={image} />

        <div className="flex flex-wrap gap-2">
          {image.media_kind === "animated_gif" ? (
            <Tag className="border-sky-300 bg-sky-50 text-sky-900">GIF</Tag>
          ) : null}
          {image.media_kind === "video_scene" ? (
            <Tag className="border-violet-300 bg-violet-50 text-violet-900">Video scene</Tag>
          ) : null}
          {image.media_kind === "audio" ? (
            <Tag className="border-emerald-300 bg-emerald-50 text-emerald-900">Audio</Tag>
          ) : null}
          {image.media_kind === "pdf_document" ? (
            <Tag className="border-red-300 bg-red-50 text-red-900">PDF document</Tag>
          ) : null}
          {image.media_kind === "pdf_page" ? (
            <Tag className="border-orange-300 bg-orange-50 text-orange-900">PDF page</Tag>
          ) : null}
          {image.ocr_text ? (
            <Tag className="border-cyan-300 bg-cyan-50 text-cyan-900">OCR</Tag>
          ) : null}
          {image.audio_analysis?.speech_detected ? (
            <Tag className="border-teal-300 bg-teal-50 text-teal-900">Speech</Tag>
          ) : null}
          {image.audio_analysis?.recognized_voices?.map((voice) => (
            <Tag className="border-lime-300 bg-lime-50 text-lime-900" key={voice.id}>
              {voice.label}
            </Tag>
          ))}
          {image.audio_analysis?.transcript_text ? (
            <Tag className="border-fuchsia-300 bg-fuchsia-50 text-fuchsia-900">Transcript</Tag>
          ) : null}
          {image.audio_analysis?.tempo_bpm ? (
            <Tag className="border-rose-300 bg-rose-50 text-rose-900">
              {image.audio_analysis.tempo_bpm.toFixed(0)} BPM
            </Tag>
          ) : null}
          {faces.length ? (
            <Tag className="border-indigo-300 bg-indigo-50 text-indigo-900">
              Faces {faces.length}
            </Tag>
          ) : null}
          {people.map((person) => (
            <Tag
              className="border-purple-300 bg-purple-50 text-purple-900"
              key={person.person_id}
              title={person.person_id}
            >
              {personDisplayName(person)}
            </Tag>
          ))}
          {result.query_scene_index !== null && result.query_scene_index !== undefined ? (
            <Tag className="border-neutral-300 bg-neutral-50 text-neutral-700">
              Query scene {result.query_scene_index + 1}
            </Tag>
          ) : null}
          {result.near_duplicate ? (
            <Tag className="border-amber-300 bg-amber-50 text-amber-900">Near duplicate</Tag>
          ) : null}
        </div>
      </div>
    </article>
  );
}

function MediaTagEditor({
  image,
  onUpdateTags,
  saving,
}: {
  image: SearchResult["image"];
  onUpdateTags?: (id: string, tags: string[]) => void;
  saving: boolean;
}) {
  const tags = image.tags ?? [];
  const [draft, setDraft] = useState(tags.join(", "));

  useEffect(() => {
    setDraft(tags.join(", "));
  }, [image.id, tags.join("\u0000")]);

  const draftTags = parseTagDraft(draft);
  const dirty = !sameTags(draftTags, tags);

  function removeTag(tag: string) {
    setDraft(draftTags.filter((item) => item !== tag).join(", "));
  }

  return (
    <form
      className="grid gap-2 rounded-md border border-neutral-200 bg-neutral-50 p-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (onUpdateTags && dirty && !saving) {
          onUpdateTags(image.id, draftTags);
        }
      }}
    >
      <div className="flex items-center justify-between gap-2">
        <Label
          className="text-xs font-semibold uppercase text-neutral-500"
          htmlFor={`tags-${image.id}`}
        >
          Tags
        </Label>
        <Button
          aria-label={`Save tags for ${image.filename}`}
          className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={!onUpdateTags || !dirty || saving}
          type="submit"
          variant="outline"
        >
          {saving ? (
            <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <Save className="size-3.5" aria-hidden="true" />
          )}
          <span>Save</span>
        </Button>
      </div>
      <Input
        aria-label={`Tags for ${image.filename}`}
        className="h-9 rounded-md border-neutral-300 bg-white text-sm"
        disabled={!onUpdateTags || saving}
        id={`tags-${image.id}`}
        onChange={(event) => setDraft(event.target.value)}
        placeholder="travel, family"
        value={draft}
      />
      {draftTags.length ? (
        <div className="flex flex-wrap gap-1.5">
          {draftTags.map((tag) => (
            <Badge
              className="inline-flex max-w-full items-center gap-1 rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-800"
              key={tag}
              variant="outline"
            >
              <span className="truncate">{tag}</span>
              <button
                aria-label={`Remove tag ${tag}`}
                className="inline-grid size-4 shrink-0 place-items-center rounded text-neutral-500 transition hover:bg-neutral-200 hover:text-neutral-950"
                disabled={!onUpdateTags || saving}
                onClick={() => removeTag(tag)}
                type="button"
              >
                <X className="size-3" aria-hidden="true" />
              </button>
            </Badge>
          ))}
        </div>
      ) : null}
    </form>
  );
}

function Tag({
  children,
  className = "",
  title,
}: {
  children: React.ReactNode;
  className?: string;
  title?: string;
}) {
  return (
    <Badge
      className={`inline-flex w-fit rounded-md border px-2 py-1 text-xs font-semibold ${className}`}
      title={title}
      variant="outline"
    >
      {children}
    </Badge>
  );
}

function parseTagDraft(value: string) {
  const seen = new Set<string>();
  const tags: string[] = [];

  for (const rawTag of value.split(",")) {
    const tag = rawTag.trim();
    const key = tag.toLocaleLowerCase();
    if (!tag || seen.has(key)) {
      continue;
    }
    seen.add(key);
    tags.push(tag);
  }

  return tags;
}

function sameTags(left: string[], right: string[]) {
  return left.length === right.length && left.every((tag, index) => tag === right[index]);
}

function formatDuration(durationMs: number) {
  return `${(durationMs / 1000).toFixed(1)}s`;
}

function formatSeconds(seconds: number) {
  return `${seconds.toFixed(1)}s`;
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

function personDisplayName(person: PersonSummary) {
  return person.label?.trim() || person.person_id;
}

function formatCaptureTime(value: string) {
  const parsed = captureTimeMs(value);
  if (parsed === null) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).format(new Date(parsed));
}

function photoCameraLabel(metadata: NonNullable<SearchResult["image"]["photo_metadata"]>) {
  return [metadata.camera_make, metadata.camera_model].filter(Boolean).join(" ") || null;
}

function formatGps(gps: NonNullable<SearchResult["image"]["photo_metadata"]>["gps"]) {
  if (!gps) {
    return "";
  }

  const coordinates = `${gps.latitude.toFixed(5)}, ${gps.longitude.toFixed(5)}`;
  return gps.altitude_meters !== null && gps.altitude_meters !== undefined
    ? `${coordinates}, ${gps.altitude_meters.toFixed(1)} m`
    : coordinates;
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-neutral-600">{label}</dt>
      <dd className="min-w-0 truncate font-medium text-neutral-900">{value}</dd>
    </div>
  );
}

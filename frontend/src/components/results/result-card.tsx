import { Button } from "@moritzbrantner/ui";
import { ImageIcon, Loader2, Trash2 } from "lucide-react";
import { formatFileSize, formatModifiedAt } from "../../lib/format";
import type { SearchResult } from "../../types";
import { AudioLinks, PdfLinks, PhotoMetadataDetails, VideoSceneLinks } from "./media-links";
import { MediaTagEditor } from "./media-tag-editor";
import { Metric } from "./metric";
import {
  formatCaptureTime,
  formatDuration,
  formatGps,
  formatPercent,
  personDisplayName,
  photoCameraLabel,
} from "./result-formatting";
import { ResultTags } from "./result-tags";

type ResultCardData = Omit<
  SearchResult,
  "hash_distance" | "near_duplicate" | "ocr_score" | "relevance_score" | "vector_score"
> & {
  duplicate_group_size?: number;
  hash_distance?: number | null;
  near_duplicate?: boolean;
  ocr_score?: number | null;
  relevance_score?: number | null;
  vector_score?: number | null;
};

export function ResultCard({
  deleting = false,
  onDelete,
  onUpdateTags,
  result,
  tagSaving = false,
}: {
  deleting?: boolean;
  onDelete?: (id: string) => void;
  onUpdateTags?: (id: string, tags: string[]) => void;
  result: ResultCardData;
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
          {typeof result.relevance_score === "number" && Number.isFinite(result.relevance_score) ? (
            <Metric label="Relevance" value={result.relevance_score.toFixed(4)} />
          ) : null}
          {typeof result.vector_score === "number" && Number.isFinite(result.vector_score) ? (
            <Metric label="Visual score" value={result.vector_score.toFixed(4)} />
          ) : null}
          {result.hash_distance !== null && result.hash_distance !== undefined ? (
            <Metric label="pHash distance" value={result.hash_distance} />
          ) : null}
          {result.duplicate_group_size && result.duplicate_group_size > 1 ? (
            <Metric label="Duplicate group" value={`${result.duplicate_group_size} media`} />
          ) : null}
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

        <ResultTags
          faces={faces}
          people={people}
          result={{
            image: result.image,
            hash_distance: result.hash_distance ?? null,
            near_duplicate: result.near_duplicate ?? false,
            ocr_score: result.ocr_score ?? null,
            query_scene_index: result.query_scene_index,
            relevance_score: result.relevance_score ?? null,
            vector_score: result.vector_score ?? 0,
          }}
        />
      </div>
    </article>
  );
}

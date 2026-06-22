import { FileAudio, FileText, FileVideo } from "lucide-react";
import type { SearchResult } from "../../types";
import { formatSeconds } from "./result-formatting";

export function VideoSceneLinks({ image }: { image: SearchResult["image"] }) {
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

export function AudioLinks({ image }: { image: SearchResult["image"] }) {
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

export function PdfLinks({ image }: { image: SearchResult["image"] }) {
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

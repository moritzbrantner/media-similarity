import { Badge } from "@moritzbrantner/ui";
import type { ReactNode } from "react";
import type { SearchResult } from "../../types";
import { personDisplayName } from "./result-formatting";

export function ResultTags({
  faces,
  people,
  result,
}: {
  faces: NonNullable<SearchResult["image"]["faces"]>;
  people: NonNullable<SearchResult["image"]["people"]>;
  result: SearchResult;
}) {
  const image = result.image;

  return (
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
      {image.ocr_text ? <Tag className="border-cyan-300 bg-cyan-50 text-cyan-900">OCR</Tag> : null}
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
        <Tag className="border-indigo-300 bg-indigo-50 text-indigo-900">Faces {faces.length}</Tag>
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
  );
}

function Tag({
  children,
  className = "",
  title,
}: {
  children: ReactNode;
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

import type { EvidenceStatus, ImagePayload } from "../../types";

function statusLabel(status: EvidenceStatus): string {
  switch (status.state) {
    case "not_requested":
      return "Not requested";
    case "unavailable":
      return "Unavailable";
    case "failed":
      return `Failed: ${status.reason}`;
    case "completed":
      return "Completed";
  }
}

export function VideoEvidenceDetails({ image }: { image: ImagePayload }) {
  const evidence = image.video_evidence;
  if (!evidence) return null;

  return (
    <div className="space-y-2 text-xs text-neutral-600">
      <p>Video OCR: {statusLabel(evidence.ocr_status)}</p>
      <details className="rounded border border-neutral-200 p-2">
        <summary className="cursor-pointer font-medium text-neutral-800">Video evidence</summary>
        <div className="mt-2 space-y-3">
          <p className="break-all">Source: {image.source_item_uri ?? image.path}</p>
          <p>Times below refer to the original source video.</p>
          {evidence.visible_text.map((text) => (
            <div key={text.id}>
              <p className="font-medium text-neutral-900">{text.text}</p>
              <p>
                Visible text · {text.start_seconds.toFixed(3)}–{text.end_seconds.toFixed(3)} s
                {text.role ? ` · ${text.role.replaceAll("_", " ")}` : ""}
              </p>
              <p>
                {text.observations.length} observation(s)
                {text.track_id ? " in one text track" : ""}
              </p>
              <p className="break-all">
                {text.producer.model} · {text.producer.model_version}
              </p>
            </div>
          ))}
          <p>Transcript evidence: {statusLabel(evidence.speech_status)}</p>
          {evidence.speech.map((speech) => (
            <div key={speech.id}>
              <p className="font-medium text-neutral-900">{speech.text}</p>
              <p>
                Speech · {speech.start_seconds.toFixed(3)}–{speech.end_seconds.toFixed(3)} s
              </p>
            </div>
          ))}
          <p className="break-all">Content SHA-256: {evidence.content_hash}</p>
        </div>
      </details>
    </div>
  );
}

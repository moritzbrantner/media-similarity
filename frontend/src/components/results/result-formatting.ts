import { captureTimeMs } from "../../search/filtering";
import type { PersonSummary, SearchResult } from "../../types";

export type PhotoMetadata = NonNullable<SearchResult["image"]["photo_metadata"]>;

export function formatDuration(durationMs: number) {
  return `${(durationMs / 1000).toFixed(1)}s`;
}

export function formatSeconds(seconds: number) {
  return `${seconds.toFixed(1)}s`;
}

export function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

export function personDisplayName(person: PersonSummary) {
  return person.label?.trim() || person.person_id;
}

export function formatCaptureTime(value: string) {
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

export function photoCameraLabel(metadata: PhotoMetadata) {
  return [metadata.camera_make, metadata.camera_model].filter(Boolean).join(" ") || null;
}

export function formatGps(gps: PhotoMetadata["gps"]) {
  if (!gps) {
    return "";
  }

  const coordinates = `${gps.latitude.toFixed(5)}, ${gps.longitude.toFixed(5)}`;
  return gps.altitude_meters !== null && gps.altitude_meters !== undefined
    ? `${coordinates}, ${gps.altitude_meters.toFixed(1)} m`
    : coordinates;
}

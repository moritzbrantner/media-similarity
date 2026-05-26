import type { SearchResult } from "../types";
import { DEFAULT_METADATA_FILTERS } from "./defaults";
import type { MetadataFilters } from "./types";

export function filterResults(results: SearchResult[], filters: MetadataFilters) {
  const cameraQuery = filters.cameraQuery.trim().toLocaleLowerCase();
  const keywordQuery = filters.keywordQuery.trim().toLocaleLowerCase();
  const nameQuery = filters.nameQuery.trim().toLocaleLowerCase();
  const personId = filters.personId.trim();
  const minSizeBytes = megabytesToBytes(positiveNumber(filters.minSizeMb));
  const maxSizeBytes = megabytesToBytes(positiveNumber(filters.maxSizeMb));
  const minWidth = positiveNumber(filters.minWidth);
  const minHeight = positiveNumber(filters.minHeight);
  const maxWidth = positiveNumber(filters.maxWidth);
  const maxHeight = positiveNumber(filters.maxHeight);
  const capturedFrom = dateBoundary(filters.captureDateFrom, "start");
  const capturedTo = dateBoundary(filters.captureDateTo, "end");
  const modifiedFrom = dateBoundary(filters.dateFrom, "start");
  const modifiedTo = dateBoundary(filters.dateTo, "end");

  return results.filter((result) => {
    const image = result.image;
    const photoMetadata = image.photo_metadata;

    if (nameQuery && !imageMatchesNameQuery(image, nameQuery)) {
      return false;
    }

    if (filters.sourceType !== "all" && image.source_type !== filters.sourceType) {
      return false;
    }

    if (filters.mediaKind !== "all" && image.media_kind !== filters.mediaKind) {
      return false;
    }

    if (cameraQuery && !photoMetadataMatchesCamera(photoMetadata, cameraQuery)) {
      return false;
    }

    if (keywordQuery && !photoMetadataMatchesKeyword(photoMetadata, keywordQuery)) {
      return false;
    }

    if (filters.hasGps === "yes" && !photoMetadata?.gps) {
      return false;
    }

    if (filters.hasGps === "no" && photoMetadata?.gps) {
      return false;
    }

    if (personId && !(image.people ?? []).some((person) => person.person_id === personId)) {
      return false;
    }

    if (filters.nearDuplicate === "only" && !result.near_duplicate) {
      return false;
    }

    if (filters.nearDuplicate === "exclude" && result.near_duplicate) {
      return false;
    }

    if (
      filters.orientation !== "all" &&
      imageOrientation(image.width, image.height) !== filters.orientation
    ) {
      return false;
    }

    if (minWidth !== null && image.width < minWidth) {
      return false;
    }

    if (minHeight !== null && image.height < minHeight) {
      return false;
    }

    if (maxWidth !== null && image.width > maxWidth) {
      return false;
    }

    if (maxHeight !== null && image.height > maxHeight) {
      return false;
    }

    if (minSizeBytes !== null && image.size_bytes < minSizeBytes) {
      return false;
    }

    if (maxSizeBytes !== null && image.size_bytes > maxSizeBytes) {
      return false;
    }

    if (capturedFrom !== null || capturedTo !== null) {
      const capturedAt = captureTimeMs(photoMetadata?.capture_time ?? null);
      if (capturedAt === null) {
        return false;
      }
      if (capturedFrom !== null && capturedAt < capturedFrom) {
        return false;
      }
      if (capturedTo !== null && capturedAt > capturedTo) {
        return false;
      }
    }

    if (modifiedFrom !== null && image.modified_at * 1000 < modifiedFrom) {
      return false;
    }

    if (modifiedTo !== null && image.modified_at * 1000 > modifiedTo) {
      return false;
    }

    return true;
  });
}

export function sourceTypesFor(results: SearchResult[], currentSourceType: string) {
  const sourceTypes = new Set(results.map((result) => result.image.source_type).filter(Boolean));
  if (currentSourceType !== "all") {
    sourceTypes.add(currentSourceType);
  }

  return Array.from(sourceTypes).sort((left, right) => left.localeCompare(right));
}

function positiveNumber(value: string) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function megabytesToBytes(value: number | null) {
  return value === null ? null : value * 1024 * 1024;
}

function dateBoundary(value: string, boundary: "end" | "start") {
  if (!value) {
    return null;
  }

  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return null;
  }

  if (boundary === "end") {
    date.setDate(date.getDate() + 1);
    date.setMilliseconds(date.getMilliseconds() - 1);
  }

  return date.getTime();
}

function imageMatchesNameQuery(image: SearchResult["image"], nameQuery: string) {
  return [image.filename, image.relative_path, image.path, image.source_uri ?? ""].some((value) =>
    value.toLocaleLowerCase().includes(nameQuery),
  );
}

function photoMetadataMatchesCamera(
  metadata: SearchResult["image"]["photo_metadata"],
  cameraQuery: string,
) {
  if (!metadata) {
    return false;
  }

  return [metadata.camera_make, metadata.camera_model, metadata.lens_model].some((value) =>
    (value ?? "").toLocaleLowerCase().includes(cameraQuery),
  );
}

function photoMetadataMatchesKeyword(
  metadata: SearchResult["image"]["photo_metadata"],
  keywordQuery: string,
) {
  return (metadata?.keywords ?? []).some((keyword) =>
    keyword.toLocaleLowerCase().includes(keywordQuery),
  );
}

export function captureTimeMs(value: string | null) {
  if (!value) {
    return null;
  }
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? null : parsed;
}

function imageOrientation(width: number, height: number): MetadataFilters["orientation"] {
  if (width === height) {
    return "square";
  }

  return width > height ? "landscape" : "portrait";
}

export function countActiveFilters(filters: MetadataFilters) {
  return Object.entries(filters).filter(([key, value]) => {
    const defaultValue = DEFAULT_METADATA_FILTERS[key as keyof MetadataFilters];
    return value !== defaultValue;
  }).length;
}

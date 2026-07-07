import type { IdentityMutationResponse, SearchResponse, SearchResult } from "../types";
import {
  DEFAULT_METADATA_FILTERS,
  DEFAULT_RESULT_SORT,
  MAX_SEARCH_HISTORY,
  SEARCH_HISTORY_STORAGE_KEY,
} from "./defaults";
import type { MetadataFilters, ResultSortMode, SearchHistoryItem } from "./types";

export function loadSearchHistory() {
  if (typeof localStorage === "undefined") {
    return [];
  }

  try {
    const stored = localStorage.getItem(SEARCH_HISTORY_STORAGE_KEY);
    const parsed: unknown = stored ? JSON.parse(stored) : [];

    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed
      .filter(isSearchHistoryItem)
      .map((item) => ({
        ...item,
        filters: normalizeMetadataFilters(item.filters),
        ocrTextQuery: stringFilter(item.ocrTextQuery),
        queryImageUrl: normalizeStoredPreviewUrl(item.queryImageUrl),
        queryMediaKind: item.queryMediaKind ?? item.response.query_media_kind ?? "static_image",
        response: normalizeSearchResponse(item.response),
        sortMode: normalizeResultSortMode(item.sortMode),
      }))
      .slice(0, MAX_SEARCH_HISTORY);
  } catch {
    return [];
  }
}

export function saveSearchHistory(history: SearchHistoryItem[]) {
  if (typeof localStorage === "undefined") {
    return;
  }

  try {
    localStorage.setItem(SEARCH_HISTORY_STORAGE_KEY, JSON.stringify(history));
  } catch {
    localStorage.removeItem(SEARCH_HISTORY_STORAGE_KEY);
  }
}

function isSearchHistoryItem(value: unknown): value is SearchHistoryItem {
  if (!value || typeof value !== "object") {
    return false;
  }

  const item = value as Partial<SearchHistoryItem>;
  const response = item.response;
  return (
    typeof item.id === "string" &&
    typeof item.fileName === "string" &&
    (item.filters === undefined || isFilterObject(item.filters)) &&
    typeof item.limit === "number" &&
    (item.ocrTextQuery === undefined || typeof item.ocrTextQuery === "string") &&
    (typeof item.queryImageUrl === "string" ||
      item.queryImageUrl === null ||
      item.queryImageUrl === undefined) &&
    (item.queryMediaKind === undefined ||
      item.queryMediaKind === "static_image" ||
      item.queryMediaKind === "animated_gif" ||
      item.queryMediaKind === "video" ||
      item.queryMediaKind === "audio" ||
      item.queryMediaKind === "pdf" ||
      item.queryMediaKind === "text") &&
    (item.sortMode === undefined || isResultSortMode(item.sortMode)) &&
    typeof item.searchedAt === "string" &&
    Boolean(response) &&
    Array.isArray(response?.results) &&
    typeof response?.count === "number" &&
    typeof response?.query_phash === "string"
  );
}

function isFilterObject(value: unknown) {
  return Boolean(value) && typeof value === "object";
}

function normalizeMetadataFilters(filters: unknown): MetadataFilters {
  if (!filters || typeof filters !== "object") {
    return DEFAULT_METADATA_FILTERS;
  }

  const partial = filters as Partial<MetadataFilters>;
  return {
    ...DEFAULT_METADATA_FILTERS,
    cameraQuery: stringFilter(partial.cameraQuery),
    captureDateFrom: stringFilter(partial.captureDateFrom),
    captureDateTo: stringFilter(partial.captureDateTo),
    dateFrom: stringFilter(partial.dateFrom),
    dateTo: stringFilter(partial.dateTo),
    hasGps: isHasGpsFilter(partial.hasGps) ? partial.hasGps : DEFAULT_METADATA_FILTERS.hasGps,
    keywordQuery: stringFilter(partial.keywordQuery),
    maxHeight: stringFilter(partial.maxHeight),
    maxSizeMb: stringFilter(partial.maxSizeMb),
    maxWidth: stringFilter(partial.maxWidth),
    mediaKind: isMediaKindFilter(partial.mediaKind)
      ? partial.mediaKind
      : DEFAULT_METADATA_FILTERS.mediaKind,
    minHeight: stringFilter(partial.minHeight),
    minSizeMb: stringFilter(partial.minSizeMb),
    minWidth: stringFilter(partial.minWidth),
    nameQuery: stringFilter(partial.nameQuery),
    nearDuplicate: isNearDuplicateFilter(partial.nearDuplicate)
      ? partial.nearDuplicate
      : DEFAULT_METADATA_FILTERS.nearDuplicate,
    orientation: isOrientationFilter(partial.orientation)
      ? partial.orientation
      : DEFAULT_METADATA_FILTERS.orientation,
    personId: stringFilter(partial.personId),
    sourceType: stringFilter(partial.sourceType) || DEFAULT_METADATA_FILTERS.sourceType,
  };
}

function stringFilter(value: unknown) {
  return typeof value === "string" ? value : "";
}

function normalizeStoredPreviewUrl(value: unknown) {
  if (typeof value !== "string" || value.startsWith("blob:")) {
    return null;
  }

  return value;
}

function isMediaKindFilter(value: unknown): value is MetadataFilters["mediaKind"] {
  return (
    value === "all" ||
    value === "static_image" ||
    value === "animated_gif" ||
    value === "video_scene" ||
    value === "audio" ||
    value === "pdf_page" ||
    value === "pdf_document"
  );
}

function isNearDuplicateFilter(value: unknown): value is MetadataFilters["nearDuplicate"] {
  return value === "all" || value === "exclude" || value === "only";
}

function isOrientationFilter(value: unknown): value is MetadataFilters["orientation"] {
  return value === "all" || value === "landscape" || value === "portrait" || value === "square";
}

function isHasGpsFilter(value: unknown): value is MetadataFilters["hasGps"] {
  return value === "all" || value === "yes" || value === "no";
}

function normalizeSearchResponse(response: SearchHistoryItem["response"]): SearchResponse {
  return {
    ...response,
    results: Array.isArray(response.results) ? response.results.map(normalizeSearchResult) : [],
    query_audio_analysis: response.query_audio_analysis ?? null,
    query_ocr_text: response.query_ocr_text ?? "",
    query_media_kind: response.query_media_kind ?? "static_image",
    scenes: Array.isArray(response.scenes)
      ? response.scenes.map((scene) => ({
          ...scene,
          page_index: scene.page_index ?? null,
          page_number: scene.page_number ?? null,
          page_label: scene.page_label ?? null,
          results: Array.isArray(scene.results) ? scene.results.map(normalizeSearchResult) : [],
        }))
      : [],
  };
}

export function removeResultFromResponse(response: SearchResponse, id: string): SearchResponse {
  const results = response.results.filter((result) => result.image.id !== id);
  return {
    ...response,
    count: results.length,
    results,
    scenes: response.scenes.map((scene) => {
      const sceneResults = scene.results.filter((result) => result.image.id !== id);
      return {
        ...scene,
        count: sceneResults.length,
        results: sceneResults,
      };
    }),
  };
}

export function updateMediaInResponse(
  response: SearchResponse,
  media: SearchResult["image"],
): SearchResponse {
  const updateResult = (result: SearchResult): SearchResult =>
    result.image.id === media.id ? { ...result, image: media } : result;

  return {
    ...response,
    results: response.results.map(updateResult),
    scenes: response.scenes.map((scene) => ({
      ...scene,
      results: scene.results.map(updateResult),
    })),
  };
}

export function applyIdentityMutationToHistory(
  history: SearchHistoryItem[],
  mutation: IdentityMutationResponse,
): SearchHistoryItem[] {
  return history.map((item) => ({
    ...item,
    response: {
      ...item.response,
      results: item.response.results.map((result) => ({
        ...result,
        image:
          mutation.kind === "person"
            ? applyPersonMutation(result.image, mutation)
            : applySpeakerMutation(result.image, mutation),
      })),
    },
  }));
}

function normalizeSearchResult(result: SearchResult): SearchResult {
  return {
    ...result,
    image: {
      ...result.image,
      faces: Array.isArray(result.image.faces) ? result.image.faces : [],
      people: Array.isArray(result.image.people) ? result.image.people : [],
      full_pdf_url: result.image.full_pdf_url ?? null,
      pdf_page_url: result.image.pdf_page_url ?? null,
      pdf_document_id: result.image.pdf_document_id ?? null,
      pdf_page_index: result.image.pdf_page_index ?? null,
      pdf_page_number: result.image.pdf_page_number ?? null,
      pdf_page_count: result.image.pdf_page_count ?? null,
      visual_embedding_model: result.image.visual_embedding_model ?? null,
      artifacts: Array.isArray(result.image.artifacts) ? result.image.artifacts : [],
      tags: Array.isArray(result.image.tags) ? result.image.tags : [],
      photo_metadata: normalizePhotoMetadata(result.image.photo_metadata),
    },
  };
}

function applyPersonMutation(
  image: SearchResult["image"],
  mutation: IdentityMutationResponse,
): SearchResult["image"] {
  const sourceIds = new Set(mutation.source_ids);
  const targetLabel = mutation.target_label;
  const nextFaces = image.faces.map((face) => {
    if (
      face.person_id === mutation.target_id ||
      (face.person_id && sourceIds.has(face.person_id))
    ) {
      return {
        ...face,
        person_id: mutation.target_id,
        person_label: targetLabel,
      };
    }
    return face;
  });

  const people = new Map<string, SearchResult["image"]["people"][number]>();
  for (const person of image.people) {
    const nextId = sourceIds.has(person.person_id) ? mutation.target_id : person.person_id;
    const nextPerson = {
      ...person,
      label: nextId === mutation.target_id ? targetLabel : person.label,
      person_id: nextId,
    };
    const existing = people.get(nextId);
    if (existing) {
      people.set(nextId, {
        ...existing,
        confidence: Math.max(existing.confidence, nextPerson.confidence),
        face_count: existing.face_count + nextPerson.face_count,
        media_count: Math.max(existing.media_count, nextPerson.media_count),
      });
    } else {
      people.set(nextId, nextPerson);
    }
  }

  return {
    ...image,
    faces: nextFaces,
    people: [...people.values()],
  };
}

function applySpeakerMutation(
  image: SearchResult["image"],
  mutation: IdentityMutationResponse,
): SearchResult["image"] {
  if (!image.audio_analysis) {
    return image;
  }

  const sourceIds = new Set(mutation.source_ids);
  const targetLabel = mutation.target_label ?? mutation.target_id;
  const voiceWeights = new Map<string, number>();
  const recognizedVoices = new Map<
    string,
    NonNullable<SearchResult["image"]["audio_analysis"]>["recognized_voices"][number]
  >();

  for (const voice of image.audio_analysis.recognized_voices) {
    const nextId = sourceIds.has(voice.id) ? mutation.target_id : voice.id;
    const nextVoice = {
      ...voice,
      id: nextId,
      label: nextId === mutation.target_id ? targetLabel : voice.label,
    };
    const existing = recognizedVoices.get(nextId);
    if (!existing) {
      recognizedVoices.set(nextId, nextVoice);
      voiceWeights.set(nextId, Math.max(voice.segment_count, 1));
      continue;
    }

    const existingWeight = voiceWeights.get(nextId) ?? 1;
    const nextWeight = Math.max(voice.segment_count, 1);
    recognizedVoices.set(nextId, {
      ...existing,
      confidence:
        (existing.confidence * existingWeight + voice.confidence * nextWeight) /
        (existingWeight + nextWeight),
      segment_count: existing.segment_count + voice.segment_count,
      total_seconds: roundMillis(existing.total_seconds + voice.total_seconds),
    });
    voiceWeights.set(nextId, existingWeight + nextWeight);
  }

  return {
    ...image,
    audio_analysis: {
      ...image.audio_analysis,
      audio_segments: image.audio_analysis.audio_segments.map((segment) => {
        if (
          segment.speaker_id === mutation.target_id ||
          (segment.speaker_id && sourceIds.has(segment.speaker_id))
        ) {
          return {
            ...segment,
            speaker_id: mutation.target_id,
            speaker_label: targetLabel,
          };
        }
        return segment;
      }),
      recognized_voices: [...recognizedVoices.values()],
    },
  };
}

function roundMillis(value: number) {
  return Math.round(value * 1000) / 1000;
}

function normalizePhotoMetadata(metadata: SearchResult["image"]["photo_metadata"]) {
  if (!metadata || typeof metadata !== "object") {
    return null;
  }

  return {
    ...metadata,
    gps: metadata.gps
      ? {
          ...metadata.gps,
          altitude_meters: metadata.gps.altitude_meters ?? null,
        }
      : null,
    keywords: Array.isArray(metadata.keywords) ? metadata.keywords : [],
    raw: Array.isArray(metadata.raw) ? metadata.raw : [],
  };
}

function normalizeResultSortMode(value: unknown): ResultSortMode {
  return isResultSortMode(value) ? value : DEFAULT_RESULT_SORT;
}

function isResultSortMode(value: unknown): value is ResultSortMode {
  return (
    value === "captured_newest" ||
    value === "filename" ||
    value === "modified_newest" ||
    value === "phash_distance" ||
    value === "relevance" ||
    value === "size_largest" ||
    value === "vector_score"
  );
}

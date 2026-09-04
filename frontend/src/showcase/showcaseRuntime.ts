import { renderAudioSampleGallery, type AudioSample } from "./audioSamples";
import { extractAudioSignature, extractVideoSignature } from "./mediaFeatures";
import { renderSampleGallery, type RenderedSample as ImageSample } from "./samples";
import { listPersistedUploads, type PersistedUpload } from "./showcaseStorage";
import { renderVideoSampleGallery, type VideoSample } from "./videoSamples";
import { createSimilarityIndex, type SimilarityIndexClient } from "./wasm";

export type MediaKind = "audio" | "image" | "video";
export type SampleSource = "bundled" | "upload";

type ShowcaseSampleBase = {
  family: string;
  id: string;
  label: string;
  mediaUrl?: string;
  previewUrl?: string;
  source: SampleSource;
};

export type ShowcaseSample = ShowcaseSampleBase &
  ({ bytes: Uint8Array; kind: "image" } | { features: Float32Array; kind: "audio" | "video" });

export type SimilarityInput =
  | { bytes: Uint8Array; kind: "image" }
  | { features: Float32Array; kind: "features" };

export type RankedSample = {
  sample: ShowcaseSample;
  score: number;
};

export type QueryPreview = {
  id: string | null;
  kind: MediaKind;
  label: string;
  mediaUrl?: string;
  previewUrl?: string;
};

export type ModeRuntime = {
  gallery: ShowcaseSample[];
  index: SimilarityIndexClient;
};

export type ModeRuntimes = Record<MediaKind, ModeRuntime>;
export type ModeGalleries = Record<MediaKind, ShowcaseSample[]>;

export type RestoreUploadsResult = {
  errors: string[];
  restored: number;
};

export const EMPTY_GALLERIES: ModeGalleries = {
  audio: [],
  image: [],
  video: [],
};

export const MODES: Record<
  MediaKind,
  {
    accept: string;
    description: string;
    label: string;
    metric: string;
    title: string;
    uploadLabel: string;
  }
> = {
  image: {
    accept: "image/png,image/jpeg,image/webp",
    description: "A compact 12×12 RGB signature captures coarse visual composition.",
    label: "Images",
    metric: "12×12 RGB signature · mean absolute feature distance",
    title: "Pick or upload an image. Rust ranks the full local corpus.",
    uploadLabel: "Add image to local index",
  },
  audio: {
    accept: "audio/*",
    description: "Twelve time slices sample twelve frequency bands from browser-decoded audio.",
    label: "Audio",
    metric: "12 time slices × 12 spectral bands · mean absolute feature distance",
    title: "Pick or upload a sound. Rust ranks the full local corpus.",
    uploadLabel: "Add audio to local index",
  },
  video: {
    accept: "video/*",
    description: "Six uniformly sampled frames capture coarse appearance and motion over time.",
    label: "Video",
    metric: "6 frames × 12×12 RGB · mean absolute feature distance",
    title: "Pick or upload a video. Rust ranks the full local corpus.",
    uploadLabel: "Add video to local index",
  },
};

function imageSample(sample: ImageSample): ShowcaseSample {
  return {
    bytes: sample.bytes,
    family: sample.family,
    id: sample.id,
    kind: "image",
    label: sample.label,
    previewUrl: sample.previewUrl,
    source: "bundled",
  };
}

function audioSample(sample: AudioSample): ShowcaseSample {
  return {
    family: sample.family,
    features: sample.features,
    id: sample.id,
    kind: "audio",
    label: sample.label,
    mediaUrl: sample.mediaUrl,
    previewUrl: sample.previewUrl,
    source: "bundled",
  };
}

function videoSample(sample: VideoSample): ShowcaseSample {
  return {
    family: sample.family,
    features: sample.features,
    id: sample.id,
    kind: "video",
    label: sample.label,
    previewUrl: sample.previewUrl,
    source: "bundled",
  };
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

function addSample(index: SimilarityIndexClient, sample: ShowcaseSample) {
  if (sample.kind === "image") {
    index.addImage(sample.bytes);
  } else {
    index.addFeatures(sample.features);
  }
}

function releaseSampleUrls(sample: ShowcaseSample) {
  const urls = new Set([sample.mediaUrl, sample.previewUrl]);
  urls.forEach((url) => {
    if (url?.startsWith("blob:")) {
      URL.revokeObjectURL(url);
    }
  });
}

async function replaceGallery(runtime: ModeRuntime, gallery: ShowcaseSample[]) {
  const replacement = await createSimilarityIndex();

  try {
    gallery.forEach((sample) => addSample(replacement, sample));
  } catch (cause) {
    replacement.free();
    throw cause;
  }

  const previous = runtime.index;
  runtime.gallery = gallery;
  runtime.index = replacement;
  previous.free();
}

export function inputForSample(sample: ShowcaseSample): SimilarityInput {
  if (sample.kind === "image") {
    return { bytes: sample.bytes, kind: "image" };
  }
  return { features: sample.features, kind: "features" };
}

export function addSampleToRuntime(runtime: ModeRuntime, sample: ShowcaseSample) {
  addSample(runtime.index, sample);
  runtime.gallery.push(sample);
}

export async function createUploadedSample(upload: PersistedUpload): Promise<ShowcaseSample> {
  const file = new File([upload.blob], upload.label, {
    lastModified: upload.createdAt,
    type: upload.mimeType,
  });

  if (upload.kind === "image") {
    return {
      bytes: new Uint8Array(await upload.blob.arrayBuffer()),
      family: "Your uploads",
      id: upload.id,
      kind: "image",
      label: upload.label,
      previewUrl: URL.createObjectURL(upload.blob),
      source: "upload",
    };
  }

  if (upload.kind === "audio") {
    const features = await extractAudioSignature(file);
    return {
      family: "Your uploads",
      features,
      id: upload.id,
      kind: "audio",
      label: upload.label,
      mediaUrl: URL.createObjectURL(upload.blob),
      source: "upload",
    };
  }

  const features = await extractVideoSignature(file);
  return {
    family: "Your uploads",
    features,
    id: upload.id,
    kind: "video",
    label: upload.label,
    mediaUrl: URL.createObjectURL(upload.blob),
    source: "upload",
  };
}

export async function restorePersistedUploads(
  runtimes: ModeRuntimes,
): Promise<RestoreUploadsResult> {
  const uploads = await listPersistedUploads();
  const errors: string[] = [];
  let restored = 0;

  for (const upload of uploads) {
    const runtime = runtimes[upload.kind];
    if (runtime.gallery.some((sample) => sample.id === upload.id)) {
      continue;
    }

    let sample: ShowcaseSample | null = null;
    try {
      sample = await createUploadedSample(upload);
      addSampleToRuntime(runtime, sample);
      restored += 1;
    } catch (cause) {
      if (sample) {
        releaseSampleUrls(sample);
      }
      errors.push(`${upload.label}: ${errorMessage(cause)}`);
    }
  }

  return { errors, restored };
}

export async function removeUploadedSample(runtime: ModeRuntime, id: string): Promise<boolean> {
  const sample = runtime.gallery.find((candidate) => candidate.id === id);
  if (!sample || sample.source !== "upload") {
    return false;
  }

  await replaceGallery(
    runtime,
    runtime.gallery.filter((candidate) => candidate.id !== id),
  );
  releaseSampleUrls(sample);
  return true;
}

export async function clearUploadedSamples(runtime: ModeRuntime): Promise<number> {
  const uploads = runtime.gallery.filter((sample) => sample.source === "upload");
  if (uploads.length === 0) {
    return 0;
  }

  await replaceGallery(
    runtime,
    runtime.gallery.filter((sample) => sample.source === "bundled"),
  );
  uploads.forEach(releaseSampleUrls);
  return uploads.length;
}

export function rankGallery(
  runtime: ModeRuntime,
  query: SimilarityInput,
  excludedId: string | null,
): RankedSample[] {
  const hits =
    query.kind === "image"
      ? runtime.index.searchImage(query.bytes, runtime.gallery.length)
      : runtime.index.searchFeatures(query.features, runtime.gallery.length);

  return hits
    .filter((hit) => runtime.gallery[hit.index]?.id !== excludedId)
    .map((hit) => ({ sample: runtime.gallery[hit.index], score: hit.score }))
    .filter((hit): hit is RankedSample => Boolean(hit.sample));
}

export function queryForSample(sample: ShowcaseSample): QueryPreview {
  return {
    id: sample.id,
    kind: sample.kind,
    label: sample.label,
    mediaUrl: sample.mediaUrl,
    previewUrl: sample.previewUrl,
  };
}

export async function createModeRuntimes(): Promise<ModeRuntimes> {
  const galleries: ModeGalleries = {
    audio: renderAudioSampleGallery().map(audioSample),
    image: (await renderSampleGallery()).map(imageSample),
    video: renderVideoSampleGallery().map(videoSample),
  };
  const [audioIndex, imageIndex, videoIndex] = await Promise.all([
    createSimilarityIndex(),
    createSimilarityIndex(),
    createSimilarityIndex(),
  ]);
  const runtimes: ModeRuntimes = {
    audio: { gallery: galleries.audio, index: audioIndex },
    image: { gallery: galleries.image, index: imageIndex },
    video: { gallery: galleries.video, index: videoIndex },
  };

  Object.values(runtimes).forEach(({ gallery, index }) => {
    gallery.forEach((sample) => addSample(index, sample));
  });
  return runtimes;
}

export function disposeModeRuntimes(runtimes: ModeRuntimes) {
  Object.values(runtimes).forEach(({ gallery, index }) => {
    index.free();
    gallery.forEach(releaseSampleUrls);
  });
}

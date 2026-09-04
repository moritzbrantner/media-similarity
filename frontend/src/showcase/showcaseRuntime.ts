import { renderAudioSampleGallery, type AudioSample } from "./audioSamples";
import { renderSampleGallery, type RenderedSample as ImageSample } from "./samples";
import { renderVideoSampleGallery, type VideoSample } from "./videoSamples";
import { createSimilarityIndex, type SimilarityIndexClient } from "./wasm";

export type MediaKind = "audio" | "image" | "video";

export type ShowcaseSample = {
  family: string;
  id: string;
  kind: MediaKind;
  label: string;
  mediaUrl?: string;
  previewUrl: string;
} & ({ bytes: Uint8Array; kind: "image" } | { features: Float32Array; kind: "audio" | "video" });

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
    title: "Pick an image. Rust ranks the gallery.",
    uploadLabel: "Try your own image",
  },
  audio: {
    accept: "audio/*",
    description: "Twelve time slices sample twelve frequency bands from browser-decoded audio.",
    label: "Audio",
    metric: "12 time slices × 12 spectral bands · mean absolute feature distance",
    title: "Pick a sound. Rust ranks its spectral profile.",
    uploadLabel: "Try your own audio",
  },
  video: {
    accept: "video/*",
    description: "Six uniformly sampled frames capture coarse appearance and motion over time.",
    label: "Video",
    metric: "6 frames × 12×12 RGB · mean absolute feature distance",
    title: "Pick a motion sample. Rust ranks the timeline.",
    uploadLabel: "Try your own video",
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
  };
}

export function inputForSample(sample: ShowcaseSample): SimilarityInput {
  if (sample.kind === "image") {
    return { bytes: sample.bytes, kind: "image" };
  }
  return { features: sample.features, kind: "features" };
}

function addSample(index: SimilarityIndexClient, sample: ShowcaseSample) {
  if (sample.kind === "image") {
    index.addImage(sample.bytes);
  } else {
    index.addFeatures(sample.features);
  }
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
    mediaUrl: sample.kind === "audio" ? sample.mediaUrl : undefined,
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
    gallery.forEach((sample) => {
      if (sample.kind === "audio" && sample.mediaUrl?.startsWith("blob:")) {
        URL.revokeObjectURL(sample.mediaUrl);
      }
    });
  });
}

const AUDIO_BANDS = 12;
const AUDIO_SEGMENTS = 12;
const AUDIO_WINDOW_SAMPLES = 512;

export const VIDEO_FRAME_COUNT = 6;
export const VIDEO_SIGNATURE_GRID = 12;

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function audioSignatureFromAccessor(
  frameCount: number,
  sampleRate: number,
  sampleAt: (frame: number) => number,
): Float32Array {
  if (frameCount <= 0 || sampleRate <= 0) {
    throw new Error("Audio clip does not contain decodable samples");
  }

  const raw = new Float32Array(AUDIO_SEGMENTS * AUDIO_BANDS);
  const nyquist = sampleRate / 2;
  const minFrequency = Math.min(90, nyquist * 0.25);
  const maxFrequency = Math.max(minFrequency, Math.min(8_000, nyquist * 0.85));
  let maximum = 0;

  for (let segment = 0; segment < AUDIO_SEGMENTS; segment += 1) {
    const center = ((segment + 0.5) / AUDIO_SEGMENTS) * Math.max(0, frameCount - 1);

    for (let band = 0; band < AUDIO_BANDS; band += 1) {
      const ratio = band / (AUDIO_BANDS - 1);
      const frequency = minFrequency * (maxFrequency / minFrequency) ** ratio;
      const angularStep = (Math.PI * 2 * frequency) / sampleRate;
      let real = 0;
      let imaginary = 0;

      for (let offset = 0; offset < AUDIO_WINDOW_SAMPLES; offset += 1) {
        const centeredOffset = offset - (AUDIO_WINDOW_SAMPLES - 1) / 2;
        const frame = Math.min(frameCount - 1, Math.max(0, Math.round(center + centeredOffset)));
        const window = 0.5 - 0.5 * Math.cos((Math.PI * 2 * offset) / (AUDIO_WINDOW_SAMPLES - 1));
        const sample = sampleAt(frame) * window;
        const phase = angularStep * offset;
        real += sample * Math.cos(phase);
        imaginary -= sample * Math.sin(phase);
      }

      const magnitude = Math.log1p(Math.hypot(real, imaginary));
      const index = segment * AUDIO_BANDS + band;
      raw[index] = magnitude;
      maximum = Math.max(maximum, magnitude);
    }
  }

  if (maximum <= Number.EPSILON) {
    return raw;
  }

  for (let index = 0; index < raw.length; index += 1) {
    raw[index] = clamp01(raw[index] / maximum);
  }

  return raw;
}

export function audioSignatureFromSamples(samples: Float32Array, sampleRate: number): Float32Array {
  return audioSignatureFromAccessor(samples.length, sampleRate, (frame) => samples[frame] ?? 0);
}

export async function extractAudioSignature(file: File): Promise<Float32Array> {
  const context = new AudioContext();

  try {
    const buffer = await context.decodeAudioData(await file.arrayBuffer());
    const channels = Array.from({ length: buffer.numberOfChannels }, (_, channel) =>
      buffer.getChannelData(channel),
    );

    return audioSignatureFromAccessor(buffer.length, buffer.sampleRate, (frame) => {
      let mixed = 0;
      for (const channel of channels) {
        mixed += channel[frame] ?? 0;
      }
      return mixed / channels.length;
    });
  } finally {
    await context.close();
  }
}

function appendCanvasRgb(
  context: CanvasRenderingContext2D,
  target: Float32Array,
  offset: number,
): number {
  const pixels = context.getImageData(0, 0, VIDEO_SIGNATURE_GRID, VIDEO_SIGNATURE_GRID).data;
  let cursor = offset;

  for (let pixel = 0; pixel < pixels.length; pixel += 4) {
    target[cursor] = pixels[pixel] / 255;
    target[cursor + 1] = pixels[pixel + 1] / 255;
    target[cursor + 2] = pixels[pixel + 2] / 255;
    cursor += 3;
  }

  return cursor;
}

export type VideoFrameRenderer = (
  context: CanvasRenderingContext2D,
  progress: number,
  width: number,
  height: number,
) => void;

export function videoSignatureFromRenderer(renderer: VideoFrameRenderer): Float32Array {
  const canvas = document.createElement("canvas");
  canvas.width = VIDEO_SIGNATURE_GRID;
  canvas.height = VIDEO_SIGNATURE_GRID;
  const context = canvas.getContext("2d", { willReadFrequently: true });

  if (!context) {
    throw new Error("Canvas 2D is unavailable in this browser");
  }

  const valuesPerFrame = VIDEO_SIGNATURE_GRID * VIDEO_SIGNATURE_GRID * 3;
  const signature = new Float32Array(VIDEO_FRAME_COUNT * valuesPerFrame);
  let offset = 0;

  for (let frame = 0; frame < VIDEO_FRAME_COUNT; frame += 1) {
    const progress = (frame + 1) / (VIDEO_FRAME_COUNT + 1);
    context.clearRect(0, 0, canvas.width, canvas.height);
    renderer(context, progress, canvas.width, canvas.height);
    offset = appendCanvasRgb(context, signature, offset);
  }

  return signature;
}

function waitForMediaEvent(element: HTMLVideoElement, eventName: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const onReady = () => {
      cleanup();
      resolve();
    };
    const onError = () => {
      cleanup();
      reject(new Error("The browser could not decode this video"));
    };
    const cleanup = () => {
      element.removeEventListener(eventName, onReady);
      element.removeEventListener("error", onError);
    };

    element.addEventListener(eventName, onReady, { once: true });
    element.addEventListener("error", onError, { once: true });
  });
}

export async function extractVideoSignature(file: File): Promise<Float32Array> {
  const mediaUrl = URL.createObjectURL(file);
  const video = document.createElement("video");
  video.muted = true;
  video.playsInline = true;
  video.preload = "auto";
  video.src = mediaUrl;

  try {
    const loaded = waitForMediaEvent(video, "loadeddata");
    video.load();
    await loaded;

    if (!Number.isFinite(video.duration) || video.duration <= 0) {
      throw new Error("Video duration is unavailable");
    }

    const canvas = document.createElement("canvas");
    canvas.width = VIDEO_SIGNATURE_GRID;
    canvas.height = VIDEO_SIGNATURE_GRID;
    const context = canvas.getContext("2d", { willReadFrequently: true });

    if (!context) {
      throw new Error("Canvas 2D is unavailable in this browser");
    }

    const valuesPerFrame = VIDEO_SIGNATURE_GRID * VIDEO_SIGNATURE_GRID * 3;
    const signature = new Float32Array(VIDEO_FRAME_COUNT * valuesPerFrame);
    let offset = 0;

    for (let frame = 0; frame < VIDEO_FRAME_COUNT; frame += 1) {
      const target = Math.min(
        video.duration * ((frame + 1) / (VIDEO_FRAME_COUNT + 1)),
        Math.max(0, video.duration - 0.001),
      );

      if (Math.abs(video.currentTime - target) > 0.0001) {
        const seeked = waitForMediaEvent(video, "seeked");
        video.currentTime = target;
        await seeked;
      }

      context.drawImage(video, 0, 0, VIDEO_SIGNATURE_GRID, VIDEO_SIGNATURE_GRID);
      offset = appendCanvasRgb(context, signature, offset);
    }

    return signature;
  } finally {
    video.removeAttribute("src");
    video.load();
    URL.revokeObjectURL(mediaUrl);
  }
}

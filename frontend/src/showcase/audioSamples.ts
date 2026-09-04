import { audioSignatureFromSamples } from "./mediaFeatures";

export type AudioSample = {
  family: string;
  features: Float32Array;
  id: string;
  label: string;
  mediaUrl: string;
  previewUrl: string;
};

type AudioRecipe = {
  baseFrequency: number;
  family: string;
  id: string;
  label: string;
  overtoneRatio: number;
  phase: number;
  rhythmHz: number;
};

const SAMPLE_RATE = 16_000;
const DURATION_SECONDS = 2.4;

const RECIPES: AudioRecipe[] = [
  {
    id: "deep-pulse-a",
    label: "Deep pulse",
    family: "Low pulse",
    baseFrequency: 180,
    overtoneRatio: 2,
    rhythmHz: 2,
    phase: 0,
  },
  {
    id: "deep-pulse-b",
    label: "Deep pulse variation",
    family: "Low pulse",
    baseFrequency: 195,
    overtoneRatio: 2,
    rhythmHz: 2.1,
    phase: 0.16,
  },
  {
    id: "bright-chime-a",
    label: "Bright chime",
    family: "High chime",
    baseFrequency: 760,
    overtoneRatio: 1.5,
    rhythmHz: 1.25,
    phase: 0,
  },
  {
    id: "bright-chime-b",
    label: "Bright chime variation",
    family: "High chime",
    baseFrequency: 810,
    overtoneRatio: 1.5,
    rhythmHz: 1.3,
    phase: 0.2,
  },
  {
    id: "warm-chord-a",
    label: "Warm chord",
    family: "Warm chord",
    baseFrequency: 330,
    overtoneRatio: 1.5,
    rhythmHz: 0.75,
    phase: 0,
  },
  {
    id: "warm-chord-b",
    label: "Warm chord variation",
    family: "Warm chord",
    baseFrequency: 350,
    overtoneRatio: 1.5,
    rhythmHz: 0.8,
    phase: 0.12,
  },
];

function synthesize(recipe: AudioRecipe): Float32Array {
  const length = Math.round(SAMPLE_RATE * DURATION_SECONDS);
  const samples = new Float32Array(length);

  for (let frame = 0; frame < length; frame += 1) {
    const time = frame / SAMPLE_RATE;
    const rhythm =
      0.25 +
      0.75 *
        (0.5 + 0.5 * Math.sin(Math.PI * 2 * recipe.rhythmHz * time + recipe.phase)) ** 2;
    const edge = Math.min(1, time * 8, (DURATION_SECONDS - time) * 8);
    const fundamental = Math.sin(Math.PI * 2 * recipe.baseFrequency * time);
    const overtone = Math.sin(
      Math.PI * 2 * recipe.baseFrequency * recipe.overtoneRatio * time + 0.35,
    );
    const sub = Math.sin(Math.PI * recipe.baseFrequency * time + recipe.phase);
    samples[frame] = Math.max(
      -1,
      Math.min(1, edge * rhythm * (0.62 * fundamental + 0.28 * overtone + 0.1 * sub)),
    );
  }

  return samples;
}

function writeAscii(view: DataView, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}

function wavBlob(samples: Float32Array): Blob {
  const bytes = new ArrayBuffer(44 + samples.length * 2);
  const view = new DataView(bytes);
  const byteRate = SAMPLE_RATE * 2;

  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + samples.length * 2, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, SAMPLE_RATE, true);
  view.setUint32(28, byteRate, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeAscii(view, 36, "data");
  view.setUint32(40, samples.length * 2, true);

  for (let index = 0; index < samples.length; index += 1) {
    const sample = Math.max(-1, Math.min(1, samples[index]));
    view.setInt16(44 + index * 2, Math.round(sample * 0x7fff), true);
  }

  return new Blob([bytes], { type: "audio/wav" });
}

function waveformPreview(samples: Float32Array, family: string): string {
  const canvas = document.createElement("canvas");
  canvas.width = 640;
  canvas.height = 400;
  const context = canvas.getContext("2d");

  if (!context) {
    throw new Error("Canvas 2D is unavailable in this browser");
  }

  const accent =
    family === "Low pulse" ? "#4ba7c7" : family === "High chime" ? "#e3a846" : "#779b83";
  context.fillStyle = "#111827";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.strokeStyle = "#263244";
  context.lineWidth = 1;

  for (let line = 1; line < 8; line += 1) {
    const x = (line / 8) * canvas.width;
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
  }

  context.strokeStyle = accent;
  context.lineWidth = 4;
  context.beginPath();

  for (let x = 0; x < canvas.width; x += 1) {
    const start = Math.floor((x / canvas.width) * samples.length);
    const end = Math.max(
      start + 1,
      Math.floor(((x + 1) / canvas.width) * samples.length),
    );
    let peak = 0;
    for (let index = start; index < end; index += 1) {
      peak = Math.max(peak, Math.abs(samples[index] ?? 0));
    }
    const y = canvas.height / 2 - peak * canvas.height * 0.38;
    if (x === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  }

  for (let x = canvas.width - 1; x >= 0; x -= 1) {
    const start = Math.floor((x / canvas.width) * samples.length);
    const end = Math.max(
      start + 1,
      Math.floor(((x + 1) / canvas.width) * samples.length),
    );
    let peak = 0;
    for (let index = start; index < end; index += 1) {
      peak = Math.max(peak, Math.abs(samples[index] ?? 0));
    }
    context.lineTo(x, canvas.height / 2 + peak * canvas.height * 0.38);
  }

  context.closePath();
  context.globalAlpha = 0.8;
  context.fillStyle = accent;
  context.fill();
  context.globalAlpha = 1;

  return canvas.toDataURL("image/png");
}

export function renderAudioSampleGallery(): AudioSample[] {
  return RECIPES.map((recipe) => {
    const samples = synthesize(recipe);
    return {
      family: recipe.family,
      features: audioSignatureFromSamples(samples, SAMPLE_RATE),
      id: recipe.id,
      label: recipe.label,
      mediaUrl: URL.createObjectURL(wavBlob(samples)),
      previewUrl: waveformPreview(samples, recipe.family),
    };
  });
}

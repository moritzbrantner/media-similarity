import { videoSignatureFromRenderer, type VideoFrameRenderer } from "./mediaFeatures";

export type VideoSample = {
  family: string;
  features: Float32Array;
  id: string;
  label: string;
  previewUrl: string;
};

type VideoScene = "blocks" | "horizon" | "orbit";

type VideoRecipe = {
  family: string;
  id: string;
  label: string;
  scene: VideoScene;
  variant: 0 | 1;
};

const RECIPES: VideoRecipe[] = [
  {
    id: "orbit-pass-a",
    label: "Orbit pass",
    family: "Orbit motion",
    scene: "orbit",
    variant: 0,
  },
  {
    id: "orbit-pass-b",
    label: "Orbit pass variation",
    family: "Orbit motion",
    scene: "orbit",
    variant: 1,
  },
  {
    id: "horizon-pan-a",
    label: "Horizon pan",
    family: "Horizon pan",
    scene: "horizon",
    variant: 0,
  },
  {
    id: "horizon-pan-b",
    label: "Horizon pan variation",
    family: "Horizon pan",
    scene: "horizon",
    variant: 1,
  },
  {
    id: "blocks-slide-a",
    label: "Sliding blocks",
    family: "Geometric motion",
    scene: "blocks",
    variant: 0,
  },
  {
    id: "blocks-slide-b",
    label: "Sliding blocks variation",
    family: "Geometric motion",
    scene: "blocks",
    variant: 1,
  },
];

function drawOrbit(
  context: CanvasRenderingContext2D,
  progress: number,
  width: number,
  height: number,
  variant: 0 | 1,
) {
  context.fillStyle = "#11162b";
  context.fillRect(0, 0, width, height);

  for (const [x, y, radius] of [
    [0.14, 0.2, 0.012],
    [0.72, 0.18, 0.008],
    [0.84, 0.72, 0.01],
    [0.28, 0.78, 0.006],
  ] as const) {
    context.beginPath();
    context.arc(width * x, height * y, Math.max(1, width * radius), 0, Math.PI * 2);
    context.fillStyle = "#dbeafe";
    context.fill();
  }

  const adjusted = Math.min(1, Math.max(0, progress + (variant === 1 ? 0.035 : 0)));
  const x = width * (0.16 + adjusted * 0.68);
  const y = height * (0.53 + Math.sin(adjusted * Math.PI * 2) * 0.12);

  context.beginPath();
  context.arc(x, y, Math.min(width, height) * 0.13, 0, Math.PI * 2);
  context.fillStyle = variant === 0 ? "#e3a846" : "#df9d42";
  context.fill();
  context.strokeStyle = "#8fb6c9";
  context.lineWidth = Math.max(1, width * 0.012);
  context.beginPath();
  context.ellipse(x, y, width * 0.2, height * 0.055, -0.35, 0, Math.PI * 2);
  context.stroke();
}

function drawHorizon(
  context: CanvasRenderingContext2D,
  progress: number,
  width: number,
  height: number,
  variant: 0 | 1,
) {
  context.fillStyle = variant === 0 ? "#c9e6f2" : "#c3dfef";
  context.fillRect(0, 0, width, height * 0.52);
  context.fillStyle = variant === 0 ? "#4ba7c7" : "#479fc3";
  context.fillRect(0, height * 0.52, width, height * 0.48);

  const sunX = width * (0.18 + progress * 0.64);
  context.beginPath();
  context.arc(sunX, height * 0.24, Math.min(width, height) * 0.09, 0, Math.PI * 2);
  context.fillStyle = "#fff0aa";
  context.fill();

  context.strokeStyle = "#dff8fb";
  context.lineWidth = Math.max(1, height * 0.035);
  context.beginPath();
  for (let step = 0; step <= 8; step += 1) {
    const x = (step / 8) * width;
    const y = height * (0.68 + Math.sin(step * 0.9 + progress * Math.PI * 2) * 0.035);
    if (step === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  }
  context.stroke();
}

function drawBlocks(
  context: CanvasRenderingContext2D,
  progress: number,
  width: number,
  height: number,
  variant: 0 | 1,
) {
  context.fillStyle = "#efe5dc";
  context.fillRect(0, 0, width, height);

  const shift = progress * width * 0.42;
  const reverseShift = (1 - progress) * width * 0.36;
  context.fillStyle = "#dd654d";
  context.fillRect(-width * 0.14 + shift, height * 0.15, width * 0.38, height * 0.27);
  context.fillStyle = "#365b6d";
  context.fillRect(width * 0.58 - reverseShift, height * 0.58, width * 0.34, height * 0.25);

  context.beginPath();
  context.arc(
    width * (variant === 0 ? 0.68 - progress * 0.22 : 0.72 - progress * 0.24),
    height * (0.26 + progress * 0.16),
    Math.min(width, height) * 0.12,
    0,
    Math.PI * 2,
  );
  context.fillStyle = "#e3a846";
  context.fill();
}

function rendererFor(recipe: VideoRecipe): VideoFrameRenderer {
  return (context, progress, width, height) => {
    switch (recipe.scene) {
      case "orbit":
        drawOrbit(context, progress, width, height, recipe.variant);
        break;
      case "horizon":
        drawHorizon(context, progress, width, height, recipe.variant);
        break;
      case "blocks":
        drawBlocks(context, progress, width, height, recipe.variant);
        break;
    }
  };
}

function filmstripPreview(renderer: VideoFrameRenderer): string {
  const canvas = document.createElement("canvas");
  canvas.width = 640;
  canvas.height = 400;
  const context = canvas.getContext("2d");

  if (!context) {
    throw new Error("Canvas 2D is unavailable in this browser");
  }

  context.fillStyle = "#090d16";
  context.fillRect(0, 0, canvas.width, canvas.height);

  const gap = 12;
  const cellWidth = Math.floor((canvas.width - gap * 3) / 2);
  const cellHeight = Math.floor((canvas.height - gap * 3) / 2);
  const progresses = [0.15, 0.38, 0.62, 0.85];

  progresses.forEach((progress, index) => {
    const frame = document.createElement("canvas");
    frame.width = cellWidth;
    frame.height = cellHeight;
    const frameContext = frame.getContext("2d");
    if (!frameContext) {
      throw new Error("Canvas 2D is unavailable in this browser");
    }
    renderer(frameContext, progress, frame.width, frame.height);
    const column = index % 2;
    const row = Math.floor(index / 2);
    context.drawImage(frame, gap + column * (cellWidth + gap), gap + row * (cellHeight + gap));
  });

  return canvas.toDataURL("image/png");
}

export function renderVideoSampleGallery(): VideoSample[] {
  return RECIPES.map((recipe) => {
    const renderer = rendererFor(recipe);
    return {
      family: recipe.family,
      features: videoSignatureFromRenderer(renderer),
      id: recipe.id,
      label: recipe.label,
      previewUrl: filmstripPreview(renderer),
    };
  });
}

export type RenderedSample = {
  bytes: Uint8Array;
  family: string;
  id: string;
  label: string;
  previewUrl: string;
};

type Scene = "coast" | "forest" | "geometry" | "sunset";

type SampleRecipe = {
  family: string;
  id: string;
  label: string;
  scene: Scene;
  variant: 0 | 1;
};

const WIDTH = 640;
const HEIGHT = 400;

const RECIPES: SampleRecipe[] = [
  {
    id: "sunset-ridge",
    label: "Sunset ridge",
    family: "Sunset",
    scene: "sunset",
    variant: 0,
  },
  {
    id: "sunset-valley",
    label: "Sunset valley",
    family: "Sunset",
    scene: "sunset",
    variant: 1,
  },
  {
    id: "coast-calm",
    label: "Calm coast",
    family: "Coast",
    scene: "coast",
    variant: 0,
  },
  {
    id: "coast-cove",
    label: "Blue cove",
    family: "Coast",
    scene: "coast",
    variant: 1,
  },
  {
    id: "forest-hills",
    label: "Forest hills",
    family: "Forest",
    scene: "forest",
    variant: 0,
  },
  {
    id: "forest-lake",
    label: "Forest lake",
    family: "Forest",
    scene: "forest",
    variant: 1,
  },
  {
    id: "coral",
    label: "Coral poster",
    family: "Poster",
    scene: "geometry",
    variant: 0,
  },
  {
    id: "orbit",
    label: "Orbit poster",
    family: "Poster",
    scene: "geometry",
    variant: 1,
  },
];

function polygon(
  context: CanvasRenderingContext2D,
  points: Array<[number, number]>,
  fillStyle: string,
) {
  context.beginPath();
  points.forEach(([x, y], index) => {
    if (index === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  });
  context.closePath();
  context.fillStyle = fillStyle;
  context.fill();
}

function drawSunset(context: CanvasRenderingContext2D, variant: 0 | 1) {
  const sky = context.createLinearGradient(0, 0, 0, HEIGHT);
  sky.addColorStop(0, "#372f72");
  sky.addColorStop(0.55, variant === 0 ? "#ea7186" : "#df6d91");
  sky.addColorStop(1, "#f6bd73");
  context.fillStyle = sky;
  context.fillRect(0, 0, WIDTH, HEIGHT);

  context.beginPath();
  context.arc(variant === 0 ? 455 : 420, variant === 0 ? 145 : 155, 48, 0, Math.PI * 2);
  context.fillStyle = "#ffe29a";
  context.fill();

  polygon(
    context,
    variant === 0
      ? [
          [0, 315],
          [125, 205],
          [230, 298],
          [360, 188],
          [505, 303],
          [640, 230],
          [640, 400],
          [0, 400],
        ]
      : [
          [0, 300],
          [110, 225],
          [245, 305],
          [375, 205],
          [640, 245],
          [640, 400],
          [0, 400],
        ],
    "#313457",
  );
  polygon(
    context,
    [
      [0, 345],
      [175, 292],
      [315, 350],
      [470, 285],
      [640, 340],
      [640, 400],
      [0, 400],
    ],
    "#20243d",
  );
}

function drawCoast(context: CanvasRenderingContext2D, variant: 0 | 1) {
  context.fillStyle = "#bfe6f5";
  context.fillRect(0, 0, WIDTH, 205);
  context.fillStyle = variant === 0 ? "#4ba7c7" : "#479fc3";
  context.fillRect(0, 205, WIDTH, 125);
  context.fillStyle = "#efd69b";
  context.fillRect(0, 330, WIDTH, 70);

  context.beginPath();
  context.arc(variant === 0 ? 120 : 155, 90, 38, 0, Math.PI * 2);
  context.fillStyle = "#fff3bd";
  context.fill();

  context.strokeStyle = "#d7f5f8";
  context.lineWidth = 9;
  context.beginPath();
  context.moveTo(0, variant === 0 ? 292 : 282);
  context.bezierCurveTo(150, 260, 260, 315, 410, 285);
  context.bezierCurveTo(500, 265, 560, 290, 640, 275);
  context.stroke();

  polygon(
    context,
    variant === 0
      ? [
          [505, 150],
          [640, 110],
          [640, 265],
          [555, 240],
        ]
      : [
          [520, 165],
          [640, 125],
          [640, 270],
          [560, 245],
        ],
    "#4f6f70",
  );
}

function drawForest(context: CanvasRenderingContext2D, variant: 0 | 1) {
  context.fillStyle = "#cde7d6";
  context.fillRect(0, 0, WIDTH, HEIGHT);
  polygon(
    context,
    variant === 0
      ? [
          [0, 285],
          [120, 185],
          [250, 280],
          [365, 160],
          [510, 275],
          [640, 195],
          [640, 400],
          [0, 400],
        ]
      : [
          [0, 270],
          [145, 175],
          [275, 275],
          [395, 175],
          [640, 205],
          [640, 400],
          [0, 400],
        ],
    "#4e8f68",
  );
  polygon(
    context,
    [
      [0, 325],
      [125, 265],
      [255, 330],
      [420, 245],
      [640, 320],
      [640, 400],
      [0, 400],
    ],
    "#286047",
  );

  if (variant === 1) {
    context.fillStyle = "#7dc0b8";
    context.beginPath();
    context.ellipse(330, 340, 145, 28, 0, 0, Math.PI * 2);
    context.fill();
  }

  for (const [x, y] of [
    [85, 280],
    [165, 300],
    [480, 285],
    [555, 310],
  ] as const) {
    polygon(
      context,
      [
        [x, y - 70],
        [x - 28, y],
        [x + 28, y],
      ],
      "#194737",
    );
  }
}

function drawGeometry(context: CanvasRenderingContext2D, variant: 0 | 1) {
  context.fillStyle = variant === 0 ? "#f4e7da" : "#efe5dc";
  context.fillRect(0, 0, WIDTH, HEIGHT);

  context.fillStyle = "#dd654d";
  context.fillRect(variant === 0 ? 70 : 88, 72, 215, 118);
  context.fillStyle = "#365b6d";
  context.fillRect(330, variant === 0 ? 205 : 188, 230, 120);

  context.beginPath();
  context.arc(variant === 0 ? 430 : 410, 108, 72, 0, Math.PI * 2);
  context.fillStyle = "#e3a846";
  context.fill();

  context.beginPath();
  context.arc(variant === 0 ? 210 : 230, 286, 64, 0, Math.PI * 2);
  context.fillStyle = "#779b83";
  context.fill();
}

function renderRecipe(recipe: SampleRecipe): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = WIDTH;
  canvas.height = HEIGHT;
  const context = canvas.getContext("2d");

  if (!context) {
    throw new Error("Canvas 2D is unavailable in this browser");
  }

  switch (recipe.scene) {
    case "sunset":
      drawSunset(context, recipe.variant);
      break;
    case "coast":
      drawCoast(context, recipe.variant);
      break;
    case "forest":
      drawForest(context, recipe.variant);
      break;
    case "geometry":
      drawGeometry(context, recipe.variant);
      break;
  }

  return canvas;
}

async function canvasBytes(canvas: HTMLCanvasElement): Promise<Uint8Array> {
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((value) => {
      if (value) {
        resolve(value);
      } else {
        reject(new Error("Could not encode demo image"));
      }
    }, "image/png");
  });

  return new Uint8Array(await blob.arrayBuffer());
}

export async function renderSampleGallery(): Promise<RenderedSample[]> {
  return Promise.all(
    RECIPES.map(async (recipe) => {
      const canvas = renderRecipe(recipe);
      return {
        bytes: await canvasBytes(canvas),
        family: recipe.family,
        id: recipe.id,
        label: recipe.label,
        previewUrl: canvas.toDataURL("image/png"),
      };
    }),
  );
}

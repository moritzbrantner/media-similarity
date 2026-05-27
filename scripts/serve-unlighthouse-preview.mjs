import path from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, gzipSync } from "node:zlib";

const port = Number.parseInt(process.argv[2] ?? "4179", 10);
const distRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../frontend/dist");
const indexPath = path.join(distRoot, "index.html");

const apiFixtures = {
  "/api/health": {
    collection: "image_similarity_perf",
    source_dir: "/images",
    sources: ["/images"],
    status: "ok",
  },
  "/api/jobs": [],
  "/api/models": {
    models: [
      {
        active: true,
        blocking: false,
        bundle_path: "/models/visual",
        cached: true,
        configured: "xenova-clip-vit-base-patch32-onnx",
        detail: "Using model bundle `xenova-clip-vit-base-patch32-onnx`",
        label: "Visual embedding",
        options: [],
        required_action: null,
        role: "visual_embedding",
      },
      {
        active: false,
        blocking: false,
        bundle_path: null,
        cached: false,
        configured: "base.en",
        detail: "Role is disabled by configuration",
        label: "Audio transcription",
        options: [],
        required_action: null,
        role: "audio_transcription",
      },
    ],
  },
  "/api/source-config": {
    default_source_dir: "/images",
    indexing: {
      audio_extensions: [".mp3", ".wav"],
      audio_transcription_enabled: false,
      collection: "image_similarity_perf",
      face_analysis_enabled: false,
      face_cluster_threshold: 0.38,
      face_detection_min_confidence: 0.75,
      face_max_frames_per_media: 8,
      face_min_cluster_images: 2,
      gif_default_frame_delay_ms: 100,
      gif_max_decode_frames: 512,
      gif_motion_weight: 0.2,
      gif_preview_frames: 16,
      gif_sample_frames: 16,
      image_extensions: [".jpg", ".png", ".gif"],
      ocr_enabled: true,
      ocr_max_frames: 4,
      pdf_extensions: [".pdf"],
      pdf_max_pages: 100,
      pdf_render_dpi: 144,
      pdf_summary_pages: 8,
      video_extensions: [".mp4", ".mov"],
      video_frame_stride: 30,
      video_max_frames: null,
      visual_embedding_enabled: true,
      visual_embedding_model: "sentence-transformers/clip-ViT-B-32",
      visual_embedding_vector_size: 512,
    },
    media_sources_file: "config/media-sources.txt",
    media_sources_seed_file: null,
    media_sources_writable: true,
    sources: [
      {
        detail: null,
        kind: "local",
        spec: "/images",
        status: "ready",
      },
    ],
    supported_source_types: [
      {
        example: "/images or local:///images",
        implemented: true,
        kind: "local",
        label: "Local folder",
      },
    ],
  },
};

function assetPath(pathname) {
  const decodedPath = decodeURIComponent(pathname);
  const relativePath = decodedPath.startsWith("/static/")
    ? decodedPath.slice("/static/".length)
    : decodedPath.slice(1);
  const fullPath = path.resolve(distRoot, relativePath);

  if (!fullPath.startsWith(`${distRoot}${path.sep}`)) {
    return null;
  }

  return fullPath;
}

function contentType(filePath) {
  switch (path.extname(filePath)) {
    case ".css":
      return "text/css; charset=utf-8";
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    case ".svg":
      return "image/svg+xml";
    case ".webp":
      return "image/webp";
    case ".png":
      return "image/png";
    case ".jpg":
    case ".jpeg":
      return "image/jpeg";
    default:
      return "application/octet-stream";
  }
}

async function staticResponse(request, filePath, cacheControl) {
  const headers = {
    "Cache-Control": cacheControl,
    "Content-Type": contentType(filePath),
  };
  const file = Bun.file(filePath);

  if (![".css", ".html", ".js", ".json", ".svg"].includes(path.extname(filePath))) {
    return new Response(file, { headers });
  }

  const body = Buffer.from(await file.arrayBuffer());
  const acceptEncoding = request.headers.get("accept-encoding") ?? "";
  if (acceptEncoding.includes("br")) {
    return new Response(brotliCompressSync(body), {
      headers: { ...headers, "Content-Encoding": "br", Vary: "Accept-Encoding" },
    });
  }

  if (acceptEncoding.includes("gzip")) {
    return new Response(gzipSync(body), {
      headers: { ...headers, "Content-Encoding": "gzip", Vary: "Accept-Encoding" },
    });
  }

  return new Response(body, { headers });
}

Bun.serve({
  fetch: async (request) => {
    const { pathname } = new URL(request.url);

    if (pathname in apiFixtures) {
      return Response.json(apiFixtures[pathname]);
    }

    if (pathname.startsWith("/api/")) {
      return Response.json({ detail: "API unavailable during performance audit" }, { status: 404 });
    }

    if (pathname === "/favicon.ico") {
      return new Response(null, {
        headers: {
          "Cache-Control": "public, max-age=31536000, immutable",
        },
        status: 204,
      });
    }

    if (pathname !== "/") {
      const resolvedAssetPath = assetPath(pathname);
      if (resolvedAssetPath) {
        const file = Bun.file(resolvedAssetPath);
        if (await file.exists()) {
          const cacheControl = pathname.startsWith("/static/assets/")
            ? "public, max-age=31536000, immutable"
            : "no-cache";
          return staticResponse(request, resolvedAssetPath, cacheControl);
        }
      }
    }

    return staticResponse(request, indexPath, "no-cache");
  },
  hostname: "127.0.0.1",
  port,
});

console.log(`Serving frontend/dist at http://127.0.0.1:${port}`);

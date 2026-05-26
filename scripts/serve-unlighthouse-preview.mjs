import path from "node:path";
import { fileURLToPath } from "node:url";

const port = Number.parseInt(process.argv[2] ?? "4179", 10);
const distRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../frontend/dist",
);
const indexPath = path.join(distRoot, "index.html");

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

Bun.serve({
  fetch: async (request) => {
    const { pathname } = new URL(request.url);

    if (pathname.startsWith("/api/")) {
      return Response.json(
        { detail: "API unavailable during performance audit" },
        { status: 503 },
      );
    }

    if (pathname !== "/") {
      const resolvedAssetPath = assetPath(pathname);
      if (resolvedAssetPath) {
        const file = Bun.file(resolvedAssetPath);
        if (await file.exists()) {
          return new Response(file);
        }
      }
    }

    return new Response(Bun.file(indexPath), {
      headers: {
        "Content-Type": "text/html; charset=utf-8",
      },
    });
  },
  hostname: "127.0.0.1",
  port,
});

console.log(`Serving frontend/dist at http://127.0.0.1:${port}`);

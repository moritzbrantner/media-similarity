import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig(({ command, mode }) => ({
  base: command === "serve" ? "/" : mode === "github-pages" ? "/media-similarity/" : "/static/",
  plugins: [react(), tailwindcss()],
  root: "frontend",
  build: {
    emptyOutDir: true,
    outDir: "dist",
    rolldownOptions: {
      output: {
        assetFileNames: "assets/[name][extname]",
        chunkFileNames: "assets/[name].js",
        entryFileNames: "assets/[name].js",
      },
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8000",
      "/thumbnails": "http://127.0.0.1:8000",
      "/uploads": "http://127.0.0.1:8000",
    },
  },
}));

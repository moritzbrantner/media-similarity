import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig(({ command }) => ({
  base: command === "serve" ? "/" : "/static/",
  plugins: [react(), tailwindcss()],
  root: "src/frontend",
  build: {
    emptyOutDir: true,
    outDir: "../image_similarity/static",
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8000",
      "/thumbnails": "http://127.0.0.1:8000",
    },
  },
}));

import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
export const root = process.env.ACCEPTANCE_ROOT ?? "/tmp/media88-native-acceptance-data";
export const repo = process.env.MEDIA_SIMILARITY_ROOT ?? process.cwd();
export const baseURL = process.env.ACCEPTANCE_URL ?? "http://127.0.0.1:18888";
export const qdrantURL = process.env.QDRANT_URL ?? "http://127.0.0.1:6333";
export const { chromium } = await import(
  pathToFileURL(resolve(repo, "node_modules/playwright/index.mjs")).href
);

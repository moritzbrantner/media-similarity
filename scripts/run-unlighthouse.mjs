import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
process.chdir(projectRoot);

const port = process.env.UNLIGHTHOUSE_PORT ?? "4179";
const site = process.env.UNLIGHTHOUSE_SITE ?? `http://127.0.0.1:${port}`;
const distIndexPath = path.join(projectRoot, "frontend/dist/index.html");

let serverProcess = null;
let serverLog = "";
let serverExited = false;
let cleaningUp = false;

async function cleanup() {
  if (cleaningUp) {
    return;
  }

  cleaningUp = true;

  if (serverProcess && !serverExited) {
    serverProcess.kill();
    await new Promise((resolve) => {
      serverProcess.once("close", resolve);
    });
  }
}

function installSignalHandler(signal, exitCode) {
  process.once(signal, async () => {
    await cleanup();
    process.exit(exitCode);
  });
}

function spawnCommand(command, args, options = {}) {
  return spawn(command, args, {
    cwd: projectRoot,
    stdio: options.stdio ?? "inherit",
    env: process.env,
  });
}

function runCommand(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawnCommand(command, args);

    child.once("error", reject);
    child.once("close", (code, signal) => {
      resolve({ code: code ?? 1, signal });
    });
  });
}

function startPreviewServer() {
  const child = spawnCommand(process.execPath, ["scripts/serve-unlighthouse-preview.mjs", port], {
    stdio: ["ignore", "pipe", "pipe"],
  });

  child.stdout?.on("data", (chunk) => {
    serverLog += chunk;
  });
  child.stderr?.on("data", (chunk) => {
    serverLog += chunk;
  });
  child.once("exit", () => {
    serverExited = true;
  });
  child.once("error", (error) => {
    serverExited = true;
    serverLog += `${error.message}\n`;
  });

  serverProcess = child;
}

async function waitForPreviewServer() {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      const response = await fetch(site, {
        signal: AbortSignal.timeout(1_000),
      });

      if (response.ok) {
        return;
      }
    } catch {
      // Keep polling until the server is ready, exits, or the timeout expires.
    }

    if (serverExited) {
      process.stderr.write(serverLog);
      process.exit(1);
    }

    await sleep(500);
  }

  throw new Error(`${serverLog}Timed out waiting for frontend preview at ${site}`);
}

installSignalHandler("SIGINT", 130);
installSignalHandler("SIGTERM", 143);

try {
  if (!existsSync(distIndexPath)) {
    process.stderr.write(
      "frontend/dist is missing; run `bun run build` before `bun run test:perf`.\n",
    );
    process.exit(1);
  }

  if (!process.env.UNLIGHTHOUSE_SITE) {
    startPreviewServer();
    await waitForPreviewServer();
  }

  const result = await runCommand("bunx", [
    "unlighthouse-ci",
    "--config-file",
    "unlighthouse.config.ts",
    "--site",
    site,
  ]);

  await cleanup();

  if (result.signal) {
    process.kill(process.pid, result.signal);
  }

  process.exit(result.code);
} catch (error) {
  await cleanup();
  process.stderr.write(`${error instanceof Error ? error.message : error}\n`);
  process.exit(1);
}

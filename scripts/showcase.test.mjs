// Acceptance tests for the one-command showcase flow (#77).
//
// Written by an independent acceptance pass before the implementation. They exercise
// only the public interface of `scripts/showcase.sh`:
//
// - `bun run showcase` maps to `bash scripts/showcase.sh` with no arguments.
// - `bash scripts/showcase.sh --check-prereqs` probes the required and optional tools
//   only (no builds, no compose, no sample-corpus check) and exits non-zero when a
//   required tool is missing. For each missing required tool it prints a line
//   `missing: <tool>` immediately followed by a line `hint: <how to install it>`.
//   Missing optional tools print `warn: <tool> ...` and do not fail.
// - `bash scripts/showcase.sh` (no arguments) runs the same check first and exits
//   non-zero before building or starting anything when a required tool is missing.
// - `bash scripts/showcase.sh --seed-only` indexes the sample corpus through a running
//   backend at `SHOWCASE_API_URL`, waits until indexing completes, uploads a sample
//   query from `SHOWCASE_SAMPLE_DIR/queries/` to `POST /api/search` and prints
//   `sample query returned <n> result(s)`. It exits non-zero when the query returns no
//   results or the backend is unreachable within `SHOWCASE_WAIT_SECONDS`.
//
// The prerequisite tests run the script with a controlled PATH: a directory of
// symlinked base utilities (coreutils, grep, sed, awk, curl, bash) plus a directory of
// stub executables for the tools under test. The real tools on the machine are never
// visible to the script, so one tool at a time can be removed.
//
// The opt-in end-to-end test (SHOWCASE_E2E=1) runs the real `bun run showcase` against
// local services, which needs Docker/Podman, Rust, ffmpeg and network access for the
// lightweight sample corpus.
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { spawn, spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const repoRoot = resolve(import.meta.dir, "..");
const showcaseScript = join(repoRoot, "scripts", "showcase.sh");

const baseUtilities = [
  "bash",
  "sh",
  "env",
  "dirname",
  "basename",
  "cat",
  "mkdir",
  "rm",
  "uname",
  "grep",
  "sed",
  "awk",
  "head",
  "tail",
  "tr",
  "cut",
  "id",
  "sort",
  "readlink",
  "realpath",
  "ls",
  "mktemp",
  "sleep",
  "tee",
  "wc",
  "chmod",
  "find",
  "touch",
  "date",
  "printf",
  "test",
  "true",
  "false",
  "curl",
  "kill",
];

const requiredTools = [
  { tool: "bun", hint: /bun\.sh/i },
  { tool: "cargo", hint: /rustup/i },
  { tool: "ffmpeg", hint: /ffmpeg/i },
  { tool: "ffprobe", hint: /ffmpeg/i },
];

const optionalTools = ["pdfinfo", "pdftoppm", "pdftotext"];

// Verbs that build, install, download or start something. A prerequisite failure
// must stop the command before any stub is called with one of these.
const forbiddenVerbs = new Set([
  "up",
  "run",
  "build",
  "install",
  "download",
  "pull",
  "start",
  "exec",
  "dev",
  "x",
]);

let workRoot;
let baseBin;

function which(name) {
  const result = spawnSync("bash", ["-c", `command -v -- "$1"`, "bash", name], {
    encoding: "utf8",
  });
  const found = result.stdout.trim();
  return result.status === 0 && found.startsWith("/") ? found : null;
}

beforeAll(() => {
  workRoot = mkdtempSync(join(tmpdir(), "showcase-acceptance-"));
  baseBin = join(workRoot, "base-bin");
  mkdirSync(baseBin);
  for (const name of baseUtilities) {
    const target = which(name);
    if (target) symlinkSync(target, join(baseBin, name));
  }
});

afterAll(() => {
  if (workRoot) rmSync(workRoot, { recursive: true, force: true });
});

const stubBodies = {
  plain: `exit 0`,
  // Container runtime whose compose subcommand works.
  withCompose: `exit 0`,
  // Container runtime without a compose plugin: `<runtime> compose ...` fails.
  withoutCompose: `if [ "$1" = "compose" ]; then
  echo "unknown command: compose" >&2
  exit 125
fi
exit 0`,
};

/**
 * Creates a stub bin directory. `tools` maps tool name to a stub body key.
 * Every stub appends `<name> <args>` to the log file before running its body.
 */
function makeStubBin(tools) {
  const dir = mkdtempSync(join(workRoot, "stubs-"));
  const log = join(dir, "calls.log");
  writeFileSync(log, "");
  for (const [name, body] of Object.entries(tools)) {
    const path = join(dir, name);
    writeFileSync(
      path,
      `#!/bin/sh
printf '%s %s\\n' "${name}" "$*" >> "${log}"
case "$*" in
  *version*) echo "${name} stub 1.0.0" ;;
esac
${stubBodies[body]}
`,
    );
    chmodSync(path, 0o755);
  }
  return { dir, log };
}

function defaultTools() {
  return {
    bun: "plain",
    cargo: "plain",
    rustc: "plain",
    ffmpeg: "plain",
    ffprobe: "plain",
    docker: "withCompose",
    pdfinfo: "plain",
    pdftoppm: "plain",
    pdftotext: "plain",
  };
}

function toolsWithout(...names) {
  const tools = defaultTools();
  for (const name of names) delete tools[name];
  return tools;
}

function runShowcase(args, tools) {
  const stubs = makeStubBin(tools);
  const bash = join(baseBin, "bash");
  const result = spawnSync(bash, [showcaseScript, ...args], {
    cwd: repoRoot,
    encoding: "utf8",
    timeout: 60_000,
    env: {
      PATH: `${stubs.dir}:${baseBin}`,
      HOME: join(workRoot, "home"),
      TMPDIR: workRoot,
      LANG: "C",
    },
  });
  const calls = readFileSync(stubs.log, "utf8")
    .split("\n")
    .filter((line) => line.trim() !== "");
  return {
    status: result.status,
    output: `${result.stdout}${result.stderr}`,
    calls,
  };
}

function lines(output) {
  return output.split("\n").map((line) => line.trimEnd());
}

function expectMissingWithHint(output, toolPattern, hintPattern) {
  const all = lines(output);
  const index = all.findIndex((line) => toolPattern.test(line));
  if (index === -1) {
    throw new Error(`expected a line matching ${toolPattern} in output:\n${output}`);
  }
  const hint = all[index + 1] ?? "";
  if (!/^\s*hint: \S.{8,}/.test(hint)) {
    throw new Error(
      `expected a "hint: <install instructions>" line right after "${all[index]}", got "${hint}"\n${output}`,
    );
  }
  expect(hint).toMatch(hintPattern);
}

function missingLines(output) {
  return lines(output).filter((line) => line.startsWith("missing: "));
}

describe("entry point", () => {
  test("`bun run showcase` runs scripts/showcase.sh without arguments", () => {
    const pkg = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
    expect(pkg.scripts.showcase).toBe("bash scripts/showcase.sh");
    expect(pkg.scripts["showcase:check"]).toBe("bash scripts/showcase.sh --check");
    expect(pkg.scripts["showcase:dev"]).toBe("bash scripts/showcase.sh --dev");
  });

  test("README documents `bun run showcase` as the one command", () => {
    const readme = readFileSync(join(repoRoot, "README.md"), "utf8");
    expect(readme).toMatch(/bun run showcase(?![:\w-])/);
  });
});

describe("prerequisite diagnostics (--check-prereqs)", () => {
  test("passes with every required and optional tool present", () => {
    const { status, output } = runShowcase(["--check-prereqs"], defaultTools());
    expect(missingLines(output)).toEqual([]);
    expect(status).toBe(0);
  });

  test("does not build, download or start anything", () => {
    const { calls } = runShowcase(["--check-prereqs"], defaultTools());
    const offending = calls.filter((call) =>
      call
        .split(/\s+/)
        .slice(1)
        .some((word) => forbiddenVerbs.has(word)),
    );
    expect(offending).toEqual([]);
  });

  for (const { tool, hint } of requiredTools) {
    test(`fails and names ${tool} with an install hint when it is missing`, () => {
      const { status, output } = runShowcase(["--check-prereqs"], toolsWithout(tool));
      expect(status).not.toBe(0);
      expectMissingWithHint(output, new RegExp(`^missing: ${tool}\\b`), hint);
      expect(missingLines(output)).toHaveLength(1);
    });
  }

  test("fails and names compose with an install hint when no container runtime exists", () => {
    const { status, output } = runShowcase(["--check-prereqs"], toolsWithout("docker"));
    expect(status).not.toBe(0);
    expectMissingWithHint(output, /^missing: .*compose/, /docker|podman/i);
    expect(missingLines(output)).toHaveLength(1);
  });

  test("fails when docker exists but has no compose plugin and podman is absent", () => {
    const tools = defaultTools();
    tools.docker = "withoutCompose";
    const { status, output } = runShowcase(["--check-prereqs"], tools);
    expect(status).not.toBe(0);
    expectMissingWithHint(output, /^missing: .*compose/, /docker|podman/i);
  });

  test("accepts `podman compose` when docker is absent", () => {
    const tools = toolsWithout("docker");
    tools.podman = "withCompose";
    const { status, output } = runShowcase(["--check-prereqs"], tools);
    expect(missingLines(output)).toEqual([]);
    expect(status).toBe(0);
  });

  test("accepts `podman-compose` when docker has no compose plugin and podman has none either", () => {
    const tools = defaultTools();
    tools.docker = "withoutCompose";
    tools.podman = "withoutCompose";
    tools["podman-compose"] = "plain";
    const { status, output } = runShowcase(["--check-prereqs"], tools);
    expect(missingLines(output)).toEqual([]);
    expect(status).toBe(0);
  });

  test("reports every missing required tool, not only the first", () => {
    const { status, output } = runShowcase(
      ["--check-prereqs"],
      toolsWithout("cargo", "ffprobe", "docker"),
    );
    expect(status).not.toBe(0);
    expectMissingWithHint(output, /^missing: cargo\b/, /rustup/i);
    expectMissingWithHint(output, /^missing: ffprobe\b/, /ffmpeg/i);
    expectMissingWithHint(output, /^missing: .*compose/, /docker|podman/i);
    expect(missingLines(output)).toHaveLength(3);
  });

  test("only warns about missing optional PDF tools", () => {
    const { status, output } = runShowcase(["--check-prereqs"], toolsWithout(...optionalTools));
    expect(missingLines(output)).toEqual([]);
    for (const tool of optionalTools) {
      expect(output).toMatch(new RegExp(`^warn: ${tool}\\b`, "m"));
    }
    expect(status).toBe(0);
  });
});

describe("one-command run stops on missing prerequisites", () => {
  for (const missing of ["bun", "cargo", "ffmpeg", "ffprobe", "docker"]) {
    test(`exits non-zero before building anything when ${missing} is missing`, () => {
      const { status, output, calls } = runShowcase([], toolsWithout(missing));
      expect(status).not.toBe(0);
      expect(missingLines(output)).toHaveLength(1);
      const offending = calls.filter((call) =>
        call
          .split(/\s+/)
          .slice(1)
          .some((word) => forbiddenVerbs.has(word)),
      );
      expect(offending).toEqual([]);
    });
  }
});

describe("existing modes", () => {
  test("--help still prints usage that lists the modes", () => {
    const { status, output } = runShowcase(["--help"], defaultTools());
    expect(status).toBe(0);
    for (const flag of ["--check", "--dev", "--check-prereqs", "--seed-only"]) {
      expect(output).toContain(flag);
    }
  });

  test("unknown flags still exit 2", () => {
    const { status } = runShowcase(["--definitely-not-a-flag"], defaultTools());
    expect(status).toBe(2);
  });
});

// --- --seed-only against a fake backend -------------------------------------------

function sampleQueryFiles() {
  const manifest = JSON.parse(
    readFileSync(join(repoRoot, "tests/fixtures/sample-corpus/manifest.json"), "utf8"),
  );
  return manifest.assets.filter((asset) => asset.role === "query").map((asset) => asset.filename);
}

function makeSampleDir() {
  const dir = mkdtempSync(join(workRoot, "sample-"));
  for (const filename of sampleQueryFiles()) {
    const path = join(dir, filename);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, "fake query bytes");
  }
  mkdirSync(join(dir, "sources"), { recursive: true });
  writeFileSync(join(dir, "sources", "wikimedia-example.jpg"), "fake source bytes");
  return dir;
}

function startFakeBackend({ searchResults }) {
  const requests = [];
  const jobId = "index.manual.acceptance";
  const indexResponse = {
    indexed: 3,
    already_indexed: 0,
    skipped: 0,
    failed: 0,
    pruned: 0,
    collection: "image_similarity",
    source_dir: "/sample",
    errors: [],
  };
  const now = new Date().toISOString();
  const job = (status) => ({
    spec: { id: jobId, name: "Index media sources", kind: "index.manual" },
    status,
    progress: null,
    logs: [],
    artifacts: [],
    created_at: now,
    started_at: now,
    finished_at: status === "Succeeded" ? now : null,
    failure: null,
    metadata: {},
  });
  const result = {
    score: 1,
    distance: 0,
    hash_distance: 0,
    near_duplicate: true,
    image: {
      id: "fake-1",
      filename: "wikimedia-example.jpg",
      relative_path: "wikimedia-example.jpg",
      path: "/sample/sources/wikimedia-example.jpg",
      thumbnail_url: "/thumbnails/fake-1.jpg",
      media_kind: "image",
    },
  };
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    async fetch(request) {
      const url = new URL(request.url);
      requests.push(`${request.method} ${url.pathname}`);
      if (request.method === "POST") await request.arrayBuffer();
      if (url.pathname === "/api/health") return Response.json({ status: "ok" });
      if (url.pathname === "/api/ready") return Response.json({ status: "ready", checks: [] });
      if (url.pathname === "/api/index" && request.method === "POST") {
        return Response.json(indexResponse);
      }
      if (url.pathname === "/api/jobs/index" && request.method === "POST") {
        return Response.json(job("Running"));
      }
      if (url.pathname === `/api/jobs/${jobId}`) return Response.json(job("Succeeded"));
      if (url.pathname === "/api/jobs") return Response.json([job("Succeeded")]);
      if (url.pathname === "/api/search" && request.method === "POST") {
        const results = searchResults ? [result] : [];
        return Response.json({
          query_phash: "0000000000000000",
          count: results.length,
          results,
          query_media_kind: "image",
          scenes: [],
          query_ocr_text: "",
        });
      }
      return new Response("not found", { status: 404 });
    },
  });
  return { server, requests, url: `http://127.0.0.1:${server.port}` };
}

function runSeedOnly(apiUrl, sampleDir) {
  // Real PATH for curl/bun, but compose runtimes and cargo are logging stubs so the
  // test can prove --seed-only talks to the running backend only.
  const stubs = makeStubBin({ docker: "plain", podman: "plain", cargo: "plain" });
  return new Promise((resolvePromise) => {
    const child = spawn("bash", [showcaseScript, "--seed-only"], {
      cwd: repoRoot,
      env: {
        ...process.env,
        PATH: `${stubs.dir}:${process.env.PATH}`,
        SHOWCASE_API_URL: apiUrl,
        SHOWCASE_SAMPLE_DIR: sampleDir,
        SHOWCASE_WAIT_SECONDS: "5",
      },
    });
    let output = "";
    child.stdout.on("data", (chunk) => (output += chunk));
    child.stderr.on("data", (chunk) => (output += chunk));
    const timer = setTimeout(() => child.kill("SIGKILL"), 60_000);
    child.on("close", (status) => {
      clearTimeout(timer);
      const calls = readFileSync(stubs.log, "utf8")
        .split("\n")
        .filter((line) => line.trim() !== "");
      resolvePromise({ status, output, calls });
    });
  });
}

describe("seeding against a running backend (--seed-only)", () => {
  test("indexes, waits, then runs a sample query that returns results", async () => {
    const backend = startFakeBackend({ searchResults: true });
    try {
      const { status, output, calls } = await runSeedOnly(backend.url, makeSampleDir());
      expect(calls).toEqual([]);
      const indexAt = backend.requests.findIndex(
        (request) => request === "POST /api/index" || request === "POST /api/jobs/index",
      );
      const searchAt = backend.requests.lastIndexOf("POST /api/search");
      if (indexAt === -1 || searchAt === -1) {
        throw new Error(`expected index and search requests, got ${backend.requests.join(", ")}`);
      }
      expect(searchAt).toBeGreaterThan(indexAt);
      expect(output).toMatch(/sample query returned [1-9]\d* result/);
      expect(status).toBe(0);
    } finally {
      backend.server.stop(true);
    }
  }, 90_000);

  test("fails when the sample query returns no results", async () => {
    const backend = startFakeBackend({ searchResults: false });
    try {
      const { status } = await runSeedOnly(backend.url, makeSampleDir());
      expect(backend.requests).toContain("POST /api/search");
      expect(status).not.toBe(0);
    } finally {
      backend.server.stop(true);
    }
  }, 90_000);

  test("fails with the API URL in the message when the backend is unreachable", async () => {
    const probe = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch: () => new Response("") });
    const deadUrl = `http://127.0.0.1:${probe.port}`;
    probe.stop(true);
    const { status, output } = await runSeedOnly(deadUrl, makeSampleDir());
    expect(status).not.toBe(0);
    expect(output).toContain(deadUrl);
  }, 90_000);
});

// --- opt-in end-to-end run -----------------------------------------------------------

const e2e = process.env.SHOWCASE_E2E === "1";

describe.skipIf(!e2e)("end-to-end `bun run showcase` (SHOWCASE_E2E=1)", () => {
  test("ends with a running UI where a sample query returns results", async () => {
    const child = spawn("bun", ["run", "showcase"], {
      cwd: repoRoot,
      env: { ...process.env },
      detached: true,
    });
    let output = "";
    const ready = new Promise((resolvePromise, reject) => {
      const onData = (chunk) => {
        output += chunk;
        const match = output.match(/^showcase ready: (http\S+)/m);
        if (match) resolvePromise(match[1]);
      };
      child.stdout.on("data", onData);
      child.stderr.on("data", onData);
      child.on("close", (status) =>
        reject(new Error(`showcase exited early with ${status}:\n${output}`)),
      );
    });
    try {
      const uiUrl = await ready;
      expect(output).toMatch(/^sample query: /m);
      const page = await fetch(uiUrl);
      expect(page.ok).toBe(true);

      const sampleDir = process.env.SHOWCASE_SAMPLE_DIR ?? join(repoRoot, "sample-images/showcase");
      const queryPath = join(sampleDir, "queries", "wikimedia-example-query.jpg");
      expect(existsSync(queryPath)).toBe(true);
      const form = new FormData();
      form.append("file", new Blob([readFileSync(queryPath)], { type: "image/jpeg" }), "query.jpg");
      const response = await fetch(new URL("/api/search?limit=12", uiUrl), {
        method: "POST",
        body: form,
      });
      expect(response.ok).toBe(true);
      const body = await response.json();
      expect(body.results.length).toBeGreaterThanOrEqual(1);
    } finally {
      try {
        process.kill(-child.pid, "SIGINT");
      } catch {}
    }
  }, 1_800_000);
});

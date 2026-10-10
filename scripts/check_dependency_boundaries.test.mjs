// Retrospective acceptance tests for the dependency-boundary guard (#44).
//
// Added after the guard implementation (PR #43) by an independent acceptance pass.
// They exercise only the guard's public interface: running
// `python3 scripts/check_dependency_boundaries.py` and observing its exit code and
// output. The guard locates the repository from its own path, so each case copies
// the script into a temporary repository layout.
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  appendFileSync,
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const repoRoot = resolve(import.meta.dir, "..");
const guardName = "check_dependency_boundaries.py";
const guard = join(repoRoot, "scripts", guardName);

const minimalManifest = `[package]
name = "fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
audio-analysis-core = { package = "moenarch-audio-analysis-core", version = "0.1.0" }
jobs-core = { package = "moenarch-jobs-core", version = "0.1.1" }
rt = { package = "moenarch-runtime-onnx", version = "0.1.0" }
spectral = { package = "moenarch-spectral-features", version = "0.1.0" }
serde = { version = "1", features = ["derive"] }
`;

function runGuard(root) {
  const result = spawnSync("python3", ["-B", join(root, "scripts", guardName)], {
    cwd: root,
    encoding: "utf8",
  });
  return { status: result.status, output: `${result.stdout}${result.stderr}` };
}

function expectPasses(root) {
  const { status, output } = runGuard(root);
  expect(output).toContain("dependency boundaries: ok");
  expect(status).toBe(0);
}

function expectFails(root, ...fragments) {
  const { status, output } = runGuard(root);
  expect(output).toContain("dependency boundary violations");
  for (const fragment of fragments) expect(output).toContain(fragment);
  expect(status).toBe(1);
}

function makeRoot() {
  const root = mkdtempSync(join(tmpdir(), "dependency-boundaries-"));
  mkdirSync(join(root, "scripts"));
  copyFileSync(guard, join(root, "scripts", guardName));
  return root;
}

describe("dependency-boundary guard on a fixture layout", () => {
  let root;
  const write = (relative, content) => {
    const path = join(root, relative);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, content);
  };

  beforeEach(() => {
    root = makeRoot();
    write("backend/Cargo.toml", minimalManifest);
    write("backend/src/domain/mod.rs", "pub mod models;\n");
    write(
      "backend/src/domain/models.rs",
      "pub struct Item {\n    pub audio_analysis: Option<String>,\n}\n",
    );
    mkdirSync(join(root, "backend/src/workers/media"), { recursive: true });
  });

  afterEach(() => rmSync(root, { recursive: true, force: true }));

  test("clean domain passes", () => expectPasses(root));

  test("capability imports inside the media adapter pass", () => {
    write(
      "backend/src/workers/media/audio.rs",
      "use audio_analysis_core::AudioFeatures;\nuse rt::Session;\npub fn load() { let _ = text_model_runtime::load(); }\n",
    );
    expectPasses(root);
  });

  test("allowed platform crate (moenarch-jobs-core) in domain passes", () => {
    write("backend/src/domain/jobs.rs", "use jobs_core::JobId;\npub type Id = JobId;\n");
    expectPasses(root);
  });

  test("capability names in fields, comments and strings pass", () => {
    write(
      "backend/src/domain/notes.rs",
      [
        "// use audio_analysis_core::AudioFeatures;",
        "/* text_model_runtime::load() */",
        'pub const NOTE: &str = "image_analysis_core::Image";',
        "pub fn f(x: &crate::domain::models::Item) -> bool { x.audio_analysis.is_some() }",
        "",
      ].join("\n"),
    );
    expectPasses(root);
  });

  test("use of a capability crate in domain fails", () => {
    write("backend/src/domain/audio.rs", "use audio_analysis_core::AudioFeatures;\n");
    expectFails(root, "backend/src/domain/audio.rs:1", "audio_analysis_core");
  });

  test("renamed capability alias in domain fails", () => {
    write("backend/src/domain/runtime.rs", "\nuse rt::Session;\n");
    expectFails(root, "backend/src/domain/runtime.rs:2", "`rt`");
  });

  test("capability path inside an expression fails", () => {
    write(
      "backend/src/domain/text.rs",
      "pub fn load() {\n    let _model = text_model_runtime::load();\n}\n",
    );
    expectFails(root, "backend/src/domain/text.rs:2", "text_model_runtime");
  });

  test("unlisted moenarch package in a nested domain module fails", () => {
    write(
      "backend/src/domain/search/features.rs",
      "pub fn f() -> spectral::Features { todo!() }\n",
    );
    expectFails(root, "backend/src/domain/search/features.rs:1", "spectral");
  });
});

describe("dependency-boundary guard on this repository", () => {
  test("unmodified repository passes", () => expectPasses(repoRoot));

  test("injected violation in a repository copy fails", () => {
    const root = makeRoot();
    try {
      mkdirSync(join(root, "backend"));
      copyFileSync(join(repoRoot, "backend/Cargo.toml"), join(root, "backend/Cargo.toml"));
      cpSync(join(repoRoot, "backend/src/domain"), join(root, "backend/src/domain"), {
        recursive: true,
      });
      expectPasses(root);

      appendFileSync(
        join(root, "backend/src/domain/models.rs"),
        "\n#[allow(unused_imports)]\nuse runtime_onnx::Session;\n",
      );
      expectFails(root, "backend/src/domain/models.rs:", "runtime_onnx");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

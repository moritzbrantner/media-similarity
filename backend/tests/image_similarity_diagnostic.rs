use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use image::{Rgb, RgbImage};
use serde_json::json;
use uuid::Uuid;

struct DiagnosticFixture {
    root: PathBuf,
}

impl DiagnosticFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("image-sim-diagnostic-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        RgbImage::from_pixel(16, 16, Rgb([90, 140, 190]))
            .save(root.join("image.png"))
            .unwrap();
        Self { root }
    }

    fn score(&self, right: &str) -> Output {
        self.score_with_face_models(right, false)
    }

    fn score_with_face_models(&self, right: &str, active: bool) -> Output {
        fs::write(self.root.join("corrupt.onnx"), b"invalid ONNX model").unwrap();
        fs::write(
            self.root.join("pairs.json"),
            serde_json::to_vec(&json!({
                "pairs": [{ "id": "synthetic-pair", "expected": "same_image", "left": "image.png", "right": right }]
            }))
            .unwrap(),
        )
        .unwrap();
        Command::new(env!("CARGO_BIN_EXE_image_similarity_diagnostic"))
            .current_dir(&self.root)
            .env_clear()
            .env("VISUAL_EMBEDDING_ENABLED", "false")
            .env(
                "FACE_ANALYSIS_ENABLED",
                if active { "true" } else { "false" },
            )
            .env(
                "ONNX_RUNTIME_LOAD_MODE",
                if active { "invalid" } else { "file" },
            )
            .env("MODEL_BUNDLE_DIR", self.root.join("models"))
            .env("FACE_DETECTION_MODEL_PATH", self.root.join("corrupt.onnx"))
            .env("FACE_EMBEDDING_MODEL_PATH", self.root.join("corrupt.onnx"))
            .args(["--pairs", "pairs.json"])
            .output()
            .unwrap()
    }
}

impl Drop for DiagnosticFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn diagnostic_scores_a_local_pair_and_reports_the_active_fallback() {
    let fixture = DiagnosticFixture::new();
    let output = fixture.score("image.png");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("synthetic-pair"));
    assert!(report.contains("legacy-disabled"));
    assert!(report.contains("1.000000"));
    assert!(report.contains("face models inactive"));
}

#[test]
fn diagnostic_fails_when_a_pair_cannot_be_scored() {
    let fixture = DiagnosticFixture::new();
    let output = fixture.score("missing.png");
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("1 of 1 diagnostic pairs could not be scored"));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("could not load right image"));
}

#[test]
fn diagnostic_fails_when_active_face_inference_configuration_cannot_run() {
    let fixture = DiagnosticFixture::new();
    let output = fixture.score_with_face_models("image.png", true);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("1 of 1 diagnostic pairs could not be scored"));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("face model error"));
}

#[test]
fn diagnostic_rejects_an_empty_corpus() {
    let fixture = DiagnosticFixture::new();
    fs::write(fixture.root.join("empty.json"), b"{\"pairs\":[]}").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_image_similarity_diagnostic"))
        .current_dir(&fixture.root)
        .env_clear()
        .args(["--pairs", "empty.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("no scoreable diagnostic pairs"));
}

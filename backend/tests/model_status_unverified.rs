//! Acceptance for #81 (behavior from #80): the unverified model status path
//! used by `/api/ready` and the other preflight callers never hashes bundle
//! files, yet still reports a checksum mismatch that an earlier full check
//! found; and concurrent cold full checks hash a shared file only once.
//!
//! Every test fabricates bundles on disk and denies model network access.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::Value;
use uuid::Uuid;

use image_similarity_service::config::Settings;
use image_similarity_service::workers::media::models::{
    model_status, model_status_unverified, model_statuses_unverified, record_model_provenance,
    ModelRole, ModelRuntimeStatus,
};

mod support;

use support::harness::TestApp;
use support::model_bundles::{
    deny_model_network, fabricate_face_detection_bundle, fabricate_role_bundle,
};

const ONNX: &str = "face_detection_yunet_2023mar.onnx";
const MISMATCH: &str = "checksum mismatch";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("media-similarity-81-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn settings(&self) -> Settings {
        Settings {
            model_bundle_dir: self.0.join("model-bundles"),
            model_hf_cache_dir: Some(self.0.join("hf-cache")),
            visual_embedding_enabled: false,
            face_analysis_enabled: true,
            face_detection_model_path: self.0.join("missing-legacy-face-detector.onnx"),
            face_embedding_model_path: self.0.join("missing-legacy-face-embedder.onnx"),
            audio_transcription_enabled: false,
            ..Settings::default()
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Restores read permission on drop so temp dirs can always be removed.
struct Unreadable(PathBuf);

impl Unreadable {
    /// `None` when the process can read the file anyway (e.g. running as root),
    /// in which case an unreadable file cannot be simulated.
    fn make(path: &Path) -> Option<Self> {
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
        let guard = Self(path.to_path_buf());
        if fs::File::open(path).is_ok() {
            eprintln!("skipping unreadable-file check: process can read mode 000 files");
            return None;
        }
        Some(guard)
    }
}

impl Drop for Unreadable {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o644));
    }
}

/// Mark a file as modified well before the hash-cache settle window (2 s),
/// so a full check is allowed to cache its hash.
fn settle(path: &Path) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(30))
        .unwrap();
}

/// Overwrite a file with different bytes of the same length.
fn tamper_same_size(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    for byte in &mut bytes {
        *byte ^= 0x5a;
    }
    fs::write(path, &bytes).unwrap();
}

fn detail(status: &ModelRuntimeStatus) -> &str {
    status.detail.as_deref().unwrap_or_default()
}

fn ready_check(ready: &Value, name: &str) -> Value {
    ready["checks"]
        .as_array()
        .expect("readiness checks array")
        .iter()
        .find(|check| check["name"] == name)
        .unwrap_or_else(|| panic!("missing readiness check {name}: {ready}"))
        .clone()
}

fn models_entry(models: &Value, role: &str) -> Value {
    models["models"]
        .as_array()
        .expect("models array")
        .iter()
        .find(|model| model["role"] == role)
        .unwrap_or_else(|| panic!("missing model role {role}"))
        .clone()
}

#[test]
fn unverified_status_never_opens_recorded_files_but_full_status_does() {
    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();
    let bundle = fabricate_face_detection_bundle(&settings);
    record_model_provenance(ModelRole::FaceDetection, &settings).expect("record provenance");
    let onnx = bundle.local_file(ONNX);
    let Some(_unreadable) = Unreadable::make(&onnx) else {
        return;
    };

    let unverified = model_status_unverified(ModelRole::FaceDetection, &settings);
    assert!(unverified.cached && unverified.active, "{unverified:?}");
    assert!(unverified.provenance.is_some(), "{unverified:?}");
    assert!(
        !detail(&unverified).contains(MISMATCH),
        "unverified status must not read bundle files: {unverified:?}"
    );
    let all = model_statuses_unverified(&settings);
    let face = all
        .iter()
        .find(|status| status.role == "face_detection")
        .unwrap();
    assert!(!detail(face).contains(MISMATCH), "{face:?}");

    // Control: the full path does read the file, so the unreadable fixture is
    // observable. Had the unverified path hashed, it would have failed the same way.
    let full = model_status(ModelRole::FaceDetection, &settings);
    assert!(
        detail(&full).contains(MISMATCH) && detail(&full).contains(ONNX),
        "full status must hash and fail to open the unreadable file: {full:?}"
    );
    assert!(detail(&full).contains("failed to open"), "{full:?}");
}

#[test]
fn unverified_status_reports_mismatch_only_after_a_full_check_found_it() {
    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();
    let bundle = fabricate_face_detection_bundle(&settings);
    record_model_provenance(ModelRole::FaceDetection, &settings).expect("record provenance");
    let onnx = bundle.local_file(ONNX);
    tamper_same_size(&onnx);
    settle(&onnx);

    // Same size, different bytes: only hashing can tell.
    let before = model_status_unverified(ModelRole::FaceDetection, &settings);
    assert!(
        !detail(&before).contains(MISMATCH),
        "unverified status found the tampered bytes, so it hashed: {before:?}"
    );

    let full = model_status(ModelRole::FaceDetection, &settings);
    assert!(
        detail(&full).contains(MISMATCH) && detail(&full).contains(ONNX),
        "{full:?}"
    );

    let after = model_status_unverified(ModelRole::FaceDetection, &settings);
    assert!(
        detail(&after).contains(MISMATCH) && detail(&after).contains(ONNX),
        "unverified status must report the mismatch the full check found: {after:?}"
    );
}

async fn ready_json(app: &TestApp) -> Value {
    app.raw_get("/api/ready").await.json().await.unwrap()
}

#[tokio::test]
async fn api_ready_does_not_hash_and_reports_mismatch_found_by_full_check() {
    deny_model_network();
    let app = TestApp::new(|settings| {
        settings.face_analysis_enabled = true;
        settings.visual_embedding_enabled = false;
        settings.audio_transcription_enabled = false;
    })
    .await;
    let settings = app.state.indexing_settings();
    let bundle = fabricate_face_detection_bundle(&settings);
    record_model_provenance(ModelRole::FaceDetection, &settings).expect("record provenance");
    let onnx = bundle.local_file(ONNX);

    // 1. Unreadable recorded file: /api/ready neither fails nor flags it.
    if let Some(unreadable) = Unreadable::make(&onnx) {
        let ready = ready_json(&app).await;
        let check = ready_check(&ready, "model.face_detection");
        assert_eq!(check["status"], "ok", "{check}");
        assert!(
            !check["detail"]
                .as_str()
                .unwrap_or_default()
                .contains(MISMATCH),
            "/api/ready read the unreadable bundle file: {check}"
        );
        drop(unreadable);
    }

    // 2. Same-size tampered file: /api/ready cannot know without hashing.
    tamper_same_size(&onnx);
    settle(&onnx);
    let ready = ready_json(&app).await;
    let check = ready_check(&ready, "model.face_detection");
    assert!(
        !check["detail"]
            .as_str()
            .unwrap_or_default()
            .contains(MISMATCH),
        "/api/ready hashed the tampered bundle file: {check}"
    );

    // 3. A full check (GET /api/models) finds the mismatch ...
    let models = app.get_json("/api/models").await;
    let entry = models_entry(&models, "face_detection");
    assert!(
        entry["detail"]
            .as_str()
            .unwrap_or_default()
            .contains(MISMATCH),
        "{entry}"
    );

    // ... and /api/ready now reports it from the hash cache.
    let ready = ready_json(&app).await;
    let check = ready_check(&ready, "model.face_detection");
    let check_detail = check["detail"].as_str().unwrap_or_default();
    assert!(
        check_detail.contains(MISMATCH) && check_detail.contains(ONNX),
        "/api/ready must surface the mismatch a full check found: {check}"
    );
}

/// Bytes this thread has read through read(2)-family syscalls so far
/// (`rchar` in `/proc/thread-self/io`; counts page-cache hits too). Unlike a
/// process-wide counter it is not disturbed by tests running in parallel.
#[cfg(target_os = "linux")]
fn thread_read_bytes() -> u64 {
    let io = fs::read_to_string("/proc/thread-self/io").expect("per-thread IO accounting");
    io.lines()
        .find_map(|line| line.strip_prefix("rchar:"))
        .and_then(|value| value.trim().parse().ok())
        .expect("rchar in /proc/thread-self/io")
}

/// Bytes read by `run` on the current thread.
#[cfg(target_os = "linux")]
fn bytes_read_by<T>(run: impl FnOnce() -> T) -> (T, u64) {
    let before = thread_read_bytes();
    let value = run();
    (value, thread_read_bytes() - before)
}

#[cfg(target_os = "linux")]
const WEIGHTS_LEN: u64 = 16 * 1024 * 1024;

/// Large enough that hashing it (unoptimized SHA-256 in test builds) takes far
/// longer than releasing the threads from the barrier, so callers overlap.
#[cfg(target_os = "linux")]
fn large_face_detection_bundle(settings: &Settings) -> PathBuf {
    let weights: Vec<u8> = (0..WEIGHTS_LEN as u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
        .collect();
    let bundle = fabricate_role_bundle(
        ModelRole::FaceDetection,
        settings,
        &[("config.json", b"abc"), (ONNX, &weights)],
    );
    record_model_provenance(ModelRole::FaceDetection, settings).expect("record provenance");
    let onnx = bundle.local_file(ONNX);
    settle(&onnx);
    onnx
}

/// Number of status calls that read (hashed) the whole weights file.
#[cfg(target_os = "linux")]
fn hashes(read_bytes: &[u64]) -> usize {
    read_bytes
        .iter()
        .filter(|&&bytes| bytes >= WEIGHTS_LEN)
        .count()
}

#[cfg(target_os = "linux")]
#[test]
fn concurrent_cold_full_checks_hash_a_settled_file_once() {
    use std::sync::{Arc, Barrier};

    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();
    large_face_detection_bundle(&settings);

    // The unverified path reads none of the weights.
    let (_, read) = bytes_read_by(|| model_status_unverified(ModelRole::FaceDetection, &settings));
    assert!(read < WEIGHTS_LEN, "unverified status read {read} bytes");

    const CALLERS: usize = 8;
    let barrier = Arc::new(Barrier::new(CALLERS));
    let results: Vec<(ModelRuntimeStatus, u64)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CALLERS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let settings = &settings;
                scope.spawn(move || {
                    barrier.wait();
                    bytes_read_by(|| model_status(ModelRole::FaceDetection, settings))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    for (status, _) in &results {
        assert!(status.active, "{status:?}");
        assert!(!detail(status).contains(MISMATCH), "{status:?}");
    }
    let read: Vec<u64> = results.iter().map(|(_, bytes)| *bytes).collect();
    assert_eq!(
        hashes(&read),
        1,
        "{CALLERS} concurrent cold full checks must hash the file exactly once; bytes read per caller: {read:?}"
    );

    // The settled hash is cached: a later full check does not re-read the file.
    let (again, read) = bytes_read_by(|| model_status(ModelRole::FaceDetection, &settings));
    assert!(!detail(&again).contains(MISMATCH), "{again:?}");
    assert!(
        read < WEIGHTS_LEN,
        "warm full check re-hashed ({read} bytes)"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn full_check_of_an_unsettled_file_is_not_cached() {
    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();
    let onnx = large_face_detection_bundle(&settings);
    // Touch the mtime to "now": inside the racy-clean window.
    fs::OpenOptions::new()
        .write(true)
        .open(&onnx)
        .unwrap()
        .set_modified(SystemTime::now())
        .unwrap();

    let read: Vec<u64> = (0..2)
        .map(|_| bytes_read_by(|| model_status(ModelRole::FaceDetection, &settings)).1)
        .collect();
    assert_eq!(
        hashes(&read),
        2,
        "a file modified within the settle window must be re-hashed each time: {read:?}"
    );
}

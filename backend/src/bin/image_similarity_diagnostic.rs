use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use image_similarity_service::config::Settings;
use image_similarity_service::workers::media::faces::{FaceBox, FaceDetector, FaceEmbedder};
use image_similarity_service::workers::media::image_io::load_media;
use image_similarity_service::workers::media::models::{model_status, ModelRole};
use image_similarity_service::workers::media::visual_embedding::{
    build_visual_embedder, LegacyColorEmbedder, VisualEmbeddingBackend,
};
use serde::Deserialize;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1).collect::<Vec<_>>())?;
    let settings = Settings::from_env().unwrap_or_default();
    let pairs = if let Some(path) = &args.pairs {
        load_private_pairs(path)?
    } else {
        let manifest = args
            .manifest
            .as_deref()
            .unwrap_or_else(|| Path::new("tests/fixtures/quality-corpus/manifest.json"));
        let image_root = args
            .image_root
            .as_deref()
            .unwrap_or_else(|| Path::new("sample-images/quality"));
        load_quality_pairs(manifest, image_root)?
    };

    let active_visual = build_visual_embedder(&settings);
    let legacy_visual = LegacyColorEmbedder::new(
        format!("legacy-diagnostic:{}", settings.clip_model_name),
        settings.visual_embedding_vector_size,
    );
    let face_detection_status = model_status(ModelRole::FaceDetection, &settings);
    let face_embedding_status = model_status(ModelRole::FaceEmbedding, &settings);
    let face_active = face_detection_status.active && face_embedding_status.active;
    let face_detector = face_active.then(|| FaceDetector::new(&settings));
    let face_embedder = face_active.then(|| FaceEmbedder::new(&settings));

    println!("# Image Similarity Diagnostic");
    println!();
    println!("## Model Status");
    println!();
    println!("| role | configured | active | cached | detail |");
    println!("| --- | --- | --- | --- | --- |");
    for role in [
        ModelRole::VisualEmbedding,
        ModelRole::FaceDetection,
        ModelRole::FaceEmbedding,
    ] {
        let status = model_status(role, &settings);
        println!(
            "| {} | {} | {} | {} | {} |",
            status.role,
            escape_table(&status.configured),
            status.active,
            status.cached,
            escape_table(status.detail.as_deref().unwrap_or(""))
        );
    }
    println!();
    println!("## Pair Scores");
    println!();
    println!(
        "| pair_id | expected | left | right | active_visual_model | active_visual_degraded | active_visual_cosine | legacy_color_cosine | face_model_cosine | notes |"
    );
    println!("| --- | --- | --- | --- | --- | --- | ---: | ---: | ---: | --- |");

    for pair in pairs {
        let result = score_pair(
            &pair,
            &settings,
            active_visual.as_ref(),
            &legacy_visual,
            face_detector.as_ref(),
            face_embedder.as_ref(),
        );
        match result {
            Ok(row) => println!("{}", row.to_markdown()),
            Err(error) => println!(
                "| {} | {} | {} | {} | {} | {} |  |  |  | {} |",
                escape_table(&pair.id),
                escape_table(&pair.expected),
                escape_table(&pair.left.display().to_string()),
                escape_table(&pair.right.display().to_string()),
                escape_table(active_visual.model_name()),
                active_visual.is_degraded(),
                escape_table(&error)
            ),
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Args {
    image_root: Option<PathBuf>,
    manifest: Option<PathBuf>,
    pairs: Option<PathBuf>,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            image_root: None,
            manifest: None,
            pairs: None,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--image-root" => {
                    parsed.image_root = Some(take_path_value(&args, index, "--image-root")?);
                    index += 2;
                }
                "--manifest" => {
                    parsed.manifest = Some(take_path_value(&args, index, "--manifest")?);
                    index += 2;
                }
                "--pairs" => {
                    parsed.pairs = Some(take_path_value(&args, index, "--pairs")?);
                    index += 2;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument `{other}`")),
            }
        }
        if parsed.pairs.is_some() && (parsed.image_root.is_some() || parsed.manifest.is_some()) {
            return Err("--pairs cannot be combined with --image-root or --manifest".to_string());
        }
        Ok(parsed)
    }
}

fn take_path_value(args: &[String], index: usize, flag: &str) -> Result<PathBuf, String> {
    args.get(index + 1)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} requires a path"))
}

fn print_help() {
    println!(
        "Usage:\n  image_similarity_diagnostic --image-root sample-images/quality --manifest tests/fixtures/quality-corpus/manifest.json\n  image_similarity_diagnostic --pairs experiments/image-similarity/private-pairs.local.json"
    );
}

#[derive(Debug, Clone)]
struct DiagnosticPair {
    id: String,
    expected: String,
    left: PathBuf,
    right: PathBuf,
}

#[derive(Deserialize)]
struct QualityManifest {
    assets: Vec<QualityAsset>,
    searches: Vec<QualitySearch>,
}

#[derive(Deserialize)]
struct QualityAsset {
    id: String,
    filename: String,
    identity: String,
}

#[derive(Deserialize)]
struct QualitySearch {
    id: String,
    query_asset: String,
    expected_top_k: Vec<String>,
    expected_non_matches: Vec<String>,
}

fn load_quality_pairs(
    manifest_path: &Path,
    image_root: &Path,
) -> Result<Vec<DiagnosticPair>, String> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let manifest: QualityManifest = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", manifest_path.display()))?;
    let assets = manifest
        .assets
        .into_iter()
        .map(|asset| (asset.id.clone(), asset))
        .collect::<BTreeMap<_, _>>();
    let mut pairs = Vec::new();
    for search in manifest.searches {
        let query = assets
            .get(&search.query_asset)
            .ok_or_else(|| format!("missing query asset `{}`", search.query_asset))?;
        for expected_id in &search.expected_top_k {
            let expected = assets
                .get(expected_id)
                .ok_or_else(|| format!("missing expected asset `{expected_id}`"))?;
            pairs.push(DiagnosticPair {
                id: format!("{}--same--{}", search.id, expected.id),
                expected: format!("same_person:{}", query.identity),
                left: image_root.join(&query.filename),
                right: image_root.join(&expected.filename),
            });
        }
        for non_match_id in &search.expected_non_matches {
            let non_match = assets
                .get(non_match_id)
                .ok_or_else(|| format!("missing non-match asset `{non_match_id}`"))?;
            pairs.push(DiagnosticPair {
                id: format!("{}--different--{}", search.id, non_match.id),
                expected: format!(
                    "different_person:{}!={}",
                    query.identity, non_match.identity
                ),
                left: image_root.join(&query.filename),
                right: image_root.join(&non_match.filename),
            });
        }
    }
    Ok(pairs)
}

#[derive(Deserialize)]
struct PrivatePairsManifest {
    pairs: Vec<PrivatePair>,
}

#[derive(Deserialize)]
struct PrivatePair {
    id: String,
    expected: String,
    left: PathBuf,
    right: PathBuf,
}

fn load_private_pairs(path: &Path) -> Result<Vec<DiagnosticPair>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let manifest: PrivatePairsManifest = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    Ok(manifest
        .pairs
        .into_iter()
        .map(|pair| DiagnosticPair {
            id: pair.id,
            expected: pair.expected,
            left: resolve_local_path(base, pair.left),
            right: resolve_local_path(base, pair.right),
        })
        .collect())
}

fn resolve_local_path(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

struct PairRow {
    pair_id: String,
    expected: String,
    left: String,
    right: String,
    active_visual_model: String,
    active_visual_degraded: bool,
    active_visual_cosine: Option<f32>,
    legacy_color_cosine: Option<f32>,
    face_model_cosine: Option<f32>,
    notes: Vec<String>,
}

impl PairRow {
    fn to_markdown(&self) -> String {
        format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            escape_table(&self.pair_id),
            escape_table(&self.expected),
            escape_table(&self.left),
            escape_table(&self.right),
            escape_table(&self.active_visual_model),
            self.active_visual_degraded,
            format_score(self.active_visual_cosine),
            format_score(self.legacy_color_cosine),
            format_score(self.face_model_cosine),
            escape_table(&self.notes.join("; "))
        )
    }
}

fn score_pair(
    pair: &DiagnosticPair,
    settings: &Settings,
    active_visual: &dyn VisualEmbeddingBackend,
    legacy_visual: &LegacyColorEmbedder,
    face_detector: Option<&FaceDetector>,
    face_embedder: Option<&FaceEmbedder>,
) -> Result<PairRow, String> {
    let left = load_media(&pair.left, settings)
        .map_err(|error| format!("could not load left image {}: {error}", pair.left.display()))?;
    let right = load_media(&pair.right, settings).map_err(|error| {
        format!(
            "could not load right image {}: {error}",
            pair.right.display()
        )
    })?;
    let mut notes = Vec::new();

    let active_left =
        active_visual.embed_media(&left.sampled_frames, settings.gif_motion_weight)?;
    let active_right =
        active_visual.embed_media(&right.sampled_frames, settings.gif_motion_weight)?;
    let legacy_left =
        legacy_visual.embed_media(&left.sampled_frames, settings.gif_motion_weight)?;
    let legacy_right =
        legacy_visual.embed_media(&right.sampled_frames, settings.gif_motion_weight)?;
    let face_model_cosine = match (face_detector, face_embedder) {
        (Some(detector), Some(embedder)) => {
            match (
                selected_face_embedding(detector, embedder, &left),
                selected_face_embedding(detector, embedder, &right),
            ) {
                (Ok(Some(left_face)), Ok(Some(right_face))) => {
                    Some(cosine(&left_face, &right_face))
                }
                (Ok(None), _) => {
                    notes.push("left face not detected".to_string());
                    None
                }
                (_, Ok(None)) => {
                    notes.push("right face not detected".to_string());
                    None
                }
                (Err(error), _) | (_, Err(error)) => {
                    notes.push(format!("face model error: {error}"));
                    None
                }
            }
        }
        _ => {
            notes.push("face models inactive".to_string());
            None
        }
    };

    Ok(PairRow {
        pair_id: pair.id.clone(),
        expected: pair.expected.clone(),
        left: pair.left.display().to_string(),
        right: pair.right.display().to_string(),
        active_visual_model: active_visual.model_name().to_string(),
        active_visual_degraded: active_visual.is_degraded(),
        active_visual_cosine: Some(cosine(&active_left, &active_right)),
        legacy_color_cosine: Some(cosine(&legacy_left, &legacy_right)),
        face_model_cosine,
        notes,
    })
}

fn selected_face_embedding(
    detector: &FaceDetector,
    embedder: &FaceEmbedder,
    media: &image_similarity_service::workers::media::media::DecodedMedia,
) -> Result<Option<Vec<f32>>, String> {
    let mut selected = None;
    let mut selected_score = f32::NEG_INFINITY;
    for frame in &media.sampled_frames {
        for detection in detector.detect(&frame.image)? {
            let bbox = FaceBox::from_shared(&detection.bbox);
            let score = bbox.width * bbox.height * detection.confidence;
            if score > selected_score {
                selected_score = score;
                selected = Some((frame.image.clone(), detection));
            }
        }
    }
    selected
        .map(|(image, detection)| embedder.embed_face(&image, &detection).map(Some))
        .unwrap_or(Ok(None))
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left, right) in left.iter().zip(right) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn format_score(score: Option<f32>) -> String {
    score.map(|score| format!("{score:.6}")).unwrap_or_default()
}

fn escape_table(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

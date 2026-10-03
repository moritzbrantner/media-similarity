use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufReader, Write};
#[cfg(unix)]
use std::os::raw::c_int;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::{AnimationDecoder, DynamicImage, Frame, ImageFormat, Rgb, RgbImage};
use image_similarity_service::api::AppState;
use image_similarity_service::config::{parse_extensions, Settings};
use image_similarity_service::domain::models::{ImagePayload, IndexResponse, SearchResult};
use image_similarity_service::storage::StoredPoint;
use image_similarity_service::workers::indexer::ImageIndexer;
use image_similarity_service::workers::media::audio::decode_audio_segments;
use image_similarity_service::workers::media::image_io::load_media_bytes;
use image_similarity_service::workers::media::media::DecodedMedia;
use image_similarity_service::workers::media::models::{
    download_role_bundle, model_status, ModelRole,
};
use image_similarity_service::workers::media::pdf::decode_pdf;
use image_similarity_service::workers::media::video::decode_video_scenes;
use image_similarity_service::workers::search::{ImageSearchService, SearchFilters};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Deserialize)]
struct Manifest {
    version: u32,
    name: String,
    description: String,
    default_output_dir: String,
    assets: Vec<Asset>,
    searches: Vec<SearchCase>,
}

#[derive(Clone, Deserialize)]
struct Asset {
    id: String,
    kind: String,
    role: String,
    filename: String,
    title: String,
    identity: String,
    download_url: Option<String>,
    page_url: Option<String>,
    license: Option<String>,
    attribution: Option<String>,
    copy_of: Option<String>,
    derivation: Option<Derivation>,
    expected_top_match: Option<String>,
    expected_top_k: Option<Vec<String>>,
    expected_non_matches: Option<Vec<String>>,
    expected_ocr_terms: Option<Vec<String>>,
    expected_transcript_terms: Option<Vec<String>>,
    capability: Option<String>,
}

#[derive(Clone, Deserialize)]
struct Derivation {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    position: Option<String>,
    start_seconds: Option<f64>,
    duration_seconds: Option<f64>,
    volume: Option<f32>,
    scale_width: Option<u32>,
    comment: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
struct SearchCase {
    id: String,
    query_asset: String,
    expected_identity: String,
    expected_top_match: String,
    expected_top_k: Vec<String>,
    expected_non_matches: Vec<String>,
    #[serde(default)]
    expected_ocr_terms: Vec<String>,
    #[serde(default)]
    expected_transcript_terms: Vec<String>,
    capability: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct CliOptions {
    output: Option<PathBuf>,
    keep_artifacts: bool,
    collection: Option<String>,
    skip_download_check: bool,
}

#[derive(Serialize)]
struct QualityReport {
    corpus: String,
    description: String,
    collection: String,
    index_response: Option<IndexResponse>,
    searches: Vec<SearchEvaluation>,
    tool_statuses: Vec<ToolStatus>,
    model_statuses: Vec<image_similarity_service::workers::media::models::ModelRuntimeStatus>,
    regression_metrics: QualityRegressionMetrics,
}

#[derive(Serialize)]
struct ToolStatus {
    name: String,
    available: bool,
    detail: Option<String>,
}

#[derive(Serialize)]
struct SearchEvaluation {
    id: String,
    capability: String,
    query_asset: String,
    expected_top_match: String,
    top_match: Option<String>,
    top_k: Vec<SearchHit>,
    expected_non_match_violations: Vec<String>,
    ocr_terms: Vec<TextTermEvaluation>,
    transcript_terms: Vec<TextTermEvaluation>,
    passed: bool,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
struct SearchHit {
    asset_id: Option<String>,
    media_id: String,
    filename: String,
    media_kind: String,
    vector_score: f32,
    relevance_score: Option<f32>,
    hash_distance: Option<u32>,
    near_duplicate: bool,
}

#[derive(Serialize)]
struct TextTermEvaluation {
    term: String,
    matched_expected_asset: bool,
    top_k: Vec<SearchHit>,
}

#[derive(Serialize)]
struct QualityRegressionMetrics {
    search_cases: usize,
    passed_search_cases: usize,
    failed_search_cases: usize,
    expected_top_k_assertions: usize,
    expected_non_match_assertions: usize,
    expected_non_match_violations: u32,
    text_term_assertions: usize,
    text_term_failures: usize,
    degraded_mode_count: u32,
    inactive_model_roles: Vec<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let command = if args.first().is_some_and(|arg| !arg.starts_with("--")) {
        args.remove(0)
    } else {
        "check".to_string()
    };
    let options = parse_options(&args)?;
    let repo_root = repo_root()?;
    let manifest_path = repo_root.join("tests/fixtures/quality-corpus/manifest.json");
    let manifest = load_manifest(&manifest_path)?;
    validate_manifest(&manifest)?;

    match command.as_str() {
        "check" => {
            println!(
                "quality corpus `{}` is valid: {} assets, {} searches",
                manifest.name,
                manifest.assets.len(),
                manifest.searches.len()
            );
        }
        "download" => {
            let output_dir = output_dir(&repo_root, &manifest, &options);
            materialize_corpus(&manifest, &output_dir)?;
            println!(
                "quality corpus `{}` ready at {}",
                manifest.name,
                output_dir.display()
            );
        }
        "download-models" => {
            download_required_model_bundles()?;
        }
        "evaluate" => {
            let output_dir = output_dir(&repo_root, &manifest, &options);
            if !options.skip_download_check {
                ensure_materialized(&manifest, &output_dir)?;
            }
            let report = evaluate_corpus(&repo_root, &manifest, &output_dir, &options)?;
            let failed = report.regression_metrics.failed_search_cases;
            write_report(&repo_root, &report)?;
            if failed > 0 {
                eprintln!(
                    "quality corpus `{}` failed {failed} search case(s); report written to benchmarks/results/quality-corpus-report.md",
                    manifest.name
                );
                exit_evaluate_process(1);
            }
            println!(
                "quality corpus `{}` evaluation passed; report written",
                manifest.name
            );
            exit_evaluate_process(0);
        }
        "help" | "--help" | "-h" => print_help(),
        other => {
            return Err(format!(
                "unknown command `{other}`\n\nRun `cargo run --manifest-path backend/Cargo.toml --bin quality_corpus -- help` for usage."
            ));
        }
    }
    Ok(())
}

fn exit_evaluate_process(code: i32) -> ! {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    exit_without_atexit(code)
}

#[cfg(unix)]
fn exit_without_atexit(code: i32) -> ! {
    extern "C" {
        fn _exit(status: c_int) -> !;
    }
    unsafe { _exit(code as c_int) }
}

#[cfg(not(unix))]
fn exit_without_atexit(code: i32) -> ! {
    std::process::exit(code)
}

fn parse_options(args: &[String]) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--output requires a path".to_string());
                };
                options.output = Some(PathBuf::from(value));
                index += 2;
            }
            "--keep-artifacts" => {
                options.keep_artifacts = true;
                index += 1;
            }
            "--collection" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--collection requires a collection name".to_string());
                };
                require_field("collection", value)?;
                options.collection = Some(value.clone());
                index += 2;
            }
            "--skip-download-check" => {
                options.skip_download_check = true;
                index += 1;
            }
            unknown => return Err(format!("unknown option `{unknown}`")),
        }
    }
    Ok(options)
}

fn repo_root() -> Result<PathBuf, String> {
    if let Ok(value) = env::var("QUALITY_REPO_ROOT") {
        let path = PathBuf::from(value);
        if path.as_os_str().is_empty() {
            return Err("QUALITY_REPO_ROOT must not be empty".to_string());
        }
        return Ok(path);
    }

    let backend_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    backend_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("could not resolve repo root from {}", backend_dir.display()))
}

fn download_required_model_bundles() -> Result<(), String> {
    let settings = Settings::from_env()?;
    for role in [
        ModelRole::VisualEmbedding,
        ModelRole::FaceDetection,
        ModelRole::FaceEmbedding,
        ModelRole::AudioTranscription,
    ] {
        println!("downloading {} model bundle", role.label());
        let bundle = download_role_bundle(role, &settings)?;
        println!(
            "{} model bundle ready at {}",
            role.label(),
            bundle.root.display()
        );
    }
    Ok(())
}

fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn output_dir(repo_root: &Path, manifest: &Manifest, options: &CliOptions) -> PathBuf {
    let output = options
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(&manifest.default_output_dir));
    if output.is_absolute() {
        output
    } else {
        repo_root.join(output)
    }
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    if manifest.version != 1 {
        return Err(format!(
            "unsupported quality corpus version {}",
            manifest.version
        ));
    }
    require_field("name", &manifest.name)?;
    require_field("description", &manifest.description)?;
    let mut ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    let mut source_kinds = BTreeSet::new();
    let mut assets_by_id = BTreeMap::new();
    for asset in &manifest.assets {
        require_field("asset id", &asset.id)?;
        require_field("asset kind", &asset.kind)?;
        require_field("asset role", &asset.role)?;
        require_field("asset filename", &asset.filename)?;
        require_field("asset title", &asset.title)?;
        require_field("asset identity", &asset.identity)?;
        if !ids.insert(asset.id.as_str()) {
            return Err(format!("duplicate asset id `{}`", asset.id));
        }
        assets_by_id.insert(asset.id.as_str(), asset);
        if !valid_asset_kind(&asset.kind) {
            return Err(format!(
                "asset `{}` has unsupported kind `{}`",
                asset.id, asset.kind
            ));
        }
        if !filename_matches_kind(&asset.filename, &asset.kind) {
            return Err(format!(
                "asset `{}` filename `{}` does not match kind `{}`",
                asset.id, asset.filename, asset.kind
            ));
        }
        if Path::new(&asset.filename).is_absolute() || asset.filename.contains("..") {
            return Err(format!(
                "asset `{}` filename must stay inside the output dir",
                asset.id
            ));
        }
        match asset.role.as_str() {
            "source" => {
                source_ids.insert(asset.id.as_str());
                source_kinds.insert(asset.kind.as_str());
                require_https("download_url", asset.download_url.as_deref(), &asset.id)?;
                require_https("page_url", asset.page_url.as_deref(), &asset.id)?;
                require_field("license", asset.license.as_deref().unwrap_or_default())?;
                require_field(
                    "attribution",
                    asset.attribution.as_deref().unwrap_or_default(),
                )?;
                if asset.derivation.is_some() {
                    return Err(format!(
                        "source asset `{}` must not have derivation",
                        asset.id
                    ));
                }
            }
            "query" => {
                let copy_of = asset
                    .copy_of
                    .as_deref()
                    .ok_or_else(|| format!("query asset `{}` needs copy_of", asset.id))?;
                require_field("copy_of", copy_of)?;
                let derivation = asset
                    .derivation
                    .as_ref()
                    .ok_or_else(|| format!("query asset `{}` needs derivation", asset.id))?;
                validate_derivation(asset, derivation)?;
                require_field(
                    "expected_top_match",
                    asset.expected_top_match.as_deref().unwrap_or_default(),
                )?;
                if asset.expected_top_k.as_ref().is_none_or(Vec::is_empty) {
                    return Err(format!("query asset `{}` needs expected_top_k", asset.id));
                }
                if asset
                    .expected_non_matches
                    .as_ref()
                    .is_none_or(Vec::is_empty)
                {
                    return Err(format!(
                        "query asset `{}` needs expected_non_matches",
                        asset.id
                    ));
                }
                require_field(
                    "capability",
                    asset.capability.as_deref().unwrap_or_default(),
                )?;
            }
            other => {
                return Err(format!(
                    "asset `{}` has unsupported role `{other}`",
                    asset.id
                ))
            }
        }
        validate_expected_terms(
            asset,
            "expected_ocr_terms",
            asset.expected_ocr_terms.as_deref(),
        )?;
        validate_expected_terms(
            asset,
            "expected_transcript_terms",
            asset.expected_transcript_terms.as_deref(),
        )?;
    }
    for required_kind in ["static_image", "animated_gif", "audio", "video", "pdf"] {
        if !source_kinds.contains(required_kind) {
            return Err(format!("missing `{required_kind}` source asset"));
        }
    }
    for asset in &manifest.assets {
        if let Some(copy_of) = asset.copy_of.as_deref() {
            let Some(source) = assets_by_id.get(copy_of) else {
                return Err(format!(
                    "asset `{}` copies unknown source `{copy_of}`",
                    asset.id
                ));
            };
            if !source_ids.contains(copy_of) {
                return Err(format!(
                    "asset `{}` copies non-source asset `{copy_of}`",
                    asset.id
                ));
            }
            if source.kind != asset.kind {
                return Err(format!(
                    "asset `{}` kind `{}` does not match copied source `{copy_of}` kind `{}`",
                    asset.id, asset.kind, source.kind
                ));
            }
        }
        for id in asset
            .expected_top_match
            .iter()
            .chain(asset.expected_top_k.iter().flatten())
            .chain(asset.expected_non_matches.iter().flatten())
        {
            if !ids.contains(id.as_str()) {
                return Err(format!(
                    "asset `{}` references missing asset `{id}`",
                    asset.id
                ));
            }
        }
    }
    for search in &manifest.searches {
        require_field("search id", &search.id)?;
        require_field("search capability", &search.capability)?;
        require_field("search expected identity", &search.expected_identity)?;
        require_field("search expected top match", &search.expected_top_match)?;
        let query = assets_by_id
            .get(search.query_asset.as_str())
            .ok_or_else(|| format!("search `{}` references missing query", search.id))?;
        if query.role != "query" {
            return Err(format!(
                "search `{}` query_asset `{}` is not a query asset",
                search.id, search.query_asset
            ));
        }
        if query.identity != search.expected_identity {
            return Err(format!(
                "search `{}` expected identity `{}` does not match query asset identity `{}`",
                search.id, search.expected_identity, query.identity
            ));
        }
        if search.expected_top_k.is_empty() || search.expected_non_matches.is_empty() {
            return Err(format!(
                "search `{}` needs expected_top_k and expected_non_matches",
                search.id
            ));
        }
        for id in search
            .expected_top_k
            .iter()
            .chain(&search.expected_non_matches)
            .chain(std::iter::once(&search.expected_top_match))
        {
            if !ids.contains(id.as_str()) {
                return Err(format!(
                    "search `{}` references missing asset `{id}`",
                    search.id
                ));
            }
        }
    }
    Ok(())
}

fn validate_expected_terms(
    asset: &Asset,
    field: &str,
    terms: Option<&[String]>,
) -> Result<(), String> {
    let Some(terms) = terms else {
        return Ok(());
    };
    if terms.is_empty() {
        return Err(format!("asset `{}` {field} must not be empty", asset.id));
    }
    if let Some(term) = terms.iter().find(|term| term.trim().is_empty()) {
        return Err(format!(
            "asset `{}` {field} contains empty term `{term}`",
            asset.id
        ));
    }
    Ok(())
}

fn validate_derivation(asset: &Asset, derivation: &Derivation) -> Result<(), String> {
    match derivation.kind.as_str() {
        "overlay_text" => {
            if asset.kind != "static_image" {
                return Err(format!(
                    "asset `{}` overlay_text requires static_image",
                    asset.id
                ));
            }
            require_field(
                "derivation.text",
                derivation.text.as_deref().unwrap_or_default(),
            )
        }
        "gif_overlay_text" => {
            if asset.kind != "animated_gif" {
                return Err(format!(
                    "asset `{}` gif_overlay_text requires animated_gif",
                    asset.id
                ));
            }
            require_field(
                "derivation.text",
                derivation.text.as_deref().unwrap_or_default(),
            )
        }
        "audio_reencode" => {
            if asset.kind != "audio" {
                return Err(format!(
                    "asset `{}` audio_reencode requires audio",
                    asset.id
                ));
            }
            require_positive_duration(asset, derivation)
        }
        "video_reencode" => {
            if asset.kind != "video" {
                return Err(format!(
                    "asset `{}` video_reencode requires video",
                    asset.id
                ));
            }
            require_positive_duration(asset, derivation)?;
            if derivation.scale_width.is_some_and(|width| width < 16) {
                return Err(format!("asset `{}` scale_width is too small", asset.id));
            }
            Ok(())
        }
        "append_pdf_comment" => {
            if asset.kind != "pdf" {
                return Err(format!(
                    "asset `{}` append_pdf_comment requires pdf",
                    asset.id
                ));
            }
            require_field(
                "derivation.comment",
                derivation.comment.as_deref().unwrap_or_default(),
            )
        }
        other => Err(format!(
            "asset `{}` has unsupported derivation `{other}`",
            asset.id
        )),
    }
}

fn require_positive_duration(asset: &Asset, derivation: &Derivation) -> Result<(), String> {
    match derivation.duration_seconds {
        Some(value) if value.is_finite() && value > 0.0 => Ok(()),
        _ => Err(format!(
            "asset `{}` derivation needs positive duration_seconds",
            asset.id
        )),
    }
}

fn valid_asset_kind(kind: &str) -> bool {
    matches!(
        kind,
        "static_image" | "animated_gif" | "audio" | "video" | "pdf"
    )
}

fn filename_matches_kind(filename: &str, kind: &str) -> bool {
    let filename = filename.to_ascii_lowercase();
    match kind {
        "static_image" => [".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tiff"]
            .iter()
            .any(|extension| filename.ends_with(extension)),
        "animated_gif" => filename.ends_with(".gif"),
        "audio" => [".ogg", ".mp3", ".wav", ".flac", ".m4a", ".aac", ".opus"]
            .iter()
            .any(|extension| filename.ends_with(extension)),
        "video" => [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
            .iter()
            .any(|extension| filename.ends_with(extension)),
        "pdf" => filename.ends_with(".pdf"),
        _ => false,
    }
}

fn require_field(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(())
    }
}

fn require_https(label: &str, value: Option<&str>, asset_id: &str) -> Result<(), String> {
    match value {
        Some(value) if value.starts_with("https://") => Ok(()),
        _ => Err(format!("asset `{asset_id}` needs https {label}")),
    }
}

fn materialize_corpus(manifest: &Manifest, output_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("failed to create {}: {error}", output_dir.display()))?;
    let assets_by_id = manifest
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset.clone()))
        .collect::<BTreeMap<_, _>>();
    let client = reqwest::blocking::Client::builder()
        .user_agent("media-similarity-quality-corpus/0.1")
        .build()
        .map_err(|error| format!("failed to build HTTP client: {error}"))?;

    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.role == "source")
    {
        let target = output_dir.join(&asset.filename);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        if target.is_file() {
            continue;
        }
        let url = asset
            .download_url
            .as_deref()
            .ok_or_else(|| format!("source asset `{}` has no download_url", asset.id))?;
        let response = client
            .get(url)
            .send()
            .map_err(|error| format!("failed to download `{}`: {error}", asset.id))?;
        if !response.status().is_success() {
            return Err(format!(
                "failed to download `{}` from {url}: HTTP {}",
                asset.id,
                response.status()
            ));
        }
        let bytes = response
            .bytes()
            .map_err(|error| format!("failed to read `{}` response: {error}", asset.id))?;
        fs::write(&target, bytes)
            .map_err(|error| format!("failed to write {}: {error}", target.display()))?;
    }

    for asset in manifest.assets.iter().filter(|asset| asset.role == "query") {
        let copy_of = asset
            .copy_of
            .as_deref()
            .ok_or_else(|| format!("query asset `{}` has no copy_of", asset.id))?;
        let source = assets_by_id
            .get(copy_of)
            .ok_or_else(|| format!("query asset `{}` copies unknown `{copy_of}`", asset.id))?;
        let source_path = output_dir.join(&source.filename);
        let target_path = output_dir.join(&asset.filename);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let derivation = asset
            .derivation
            .as_ref()
            .ok_or_else(|| format!("query asset `{}` has no derivation", asset.id))?;
        derive_query_asset(asset, derivation, &source_path, &target_path)?;
    }
    write_attribution(manifest, output_dir)?;
    Ok(())
}

fn derive_query_asset(
    asset: &Asset,
    derivation: &Derivation,
    source_path: &Path,
    target_path: &Path,
) -> Result<(), String> {
    match derivation.kind.as_str() {
        "overlay_text" => derive_overlay_text(source_path, target_path, derivation),
        "gif_overlay_text" => derive_gif_overlay_text(source_path, target_path, derivation),
        "audio_reencode" => derive_audio(source_path, target_path, derivation),
        "video_reencode" => derive_video(source_path, target_path, derivation),
        "append_pdf_comment" => derive_pdf_comment(source_path, target_path, derivation),
        other => Err(format!(
            "asset `{}` has unsupported derivation `{other}`",
            asset.id
        )),
    }
}

fn derive_overlay_text(
    source_path: &Path,
    target_path: &Path,
    derivation: &Derivation,
) -> Result<(), String> {
    let mut image = image::open(source_path)
        .map_err(|error| format!("failed to decode {}: {error}", source_path.display()))?
        .to_rgb8();
    draw_text_overlay(
        &mut image,
        derivation.text.as_deref().unwrap_or("QUALITY QUERY"),
        derivation.position.as_deref().unwrap_or("bottom_right"),
    );
    DynamicImage::ImageRgb8(image)
        .save_with_format(target_path, image_format_for_path(target_path)?)
        .map_err(|error| format!("failed to write {}: {error}", target_path.display()))
}

fn derive_gif_overlay_text(
    source_path: &Path,
    target_path: &Path,
    derivation: &Derivation,
) -> Result<(), String> {
    let file = fs::File::open(source_path)
        .map_err(|error| format!("failed to open {}: {error}", source_path.display()))?;
    let decoder = GifDecoder::new(BufReader::new(file))
        .map_err(|error| format!("failed to decode {}: {error}", source_path.display()))?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|error| format!("failed to decode GIF frames: {error}"))?;
    let output = fs::File::create(target_path)
        .map_err(|error| format!("failed to create {}: {error}", target_path.display()))?;
    let mut encoder = GifEncoder::new(output);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(|error| error.to_string())?;
    for frame in frames {
        let delay = frame.delay();
        let mut image = DynamicImage::ImageRgba8(frame.into_buffer()).to_rgb8();
        draw_text_overlay(
            &mut image,
            derivation.text.as_deref().unwrap_or("QUALITY"),
            derivation.position.as_deref().unwrap_or("bottom_right"),
        );
        let rgba = DynamicImage::ImageRgb8(image).to_rgba8();
        encoder
            .encode_frame(Frame::from_parts(rgba, 0, 0, delay))
            .map_err(|error| format!("failed to encode GIF frame: {error}"))?;
    }
    Ok(())
}

fn derive_audio(
    source_path: &Path,
    target_path: &Path,
    derivation: &Derivation,
) -> Result<(), String> {
    let mut command = Command::new("ffmpeg");
    command
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-ss")
        .arg(format!(
            "{:.3}",
            derivation.start_seconds.unwrap_or(0.0).max(0.0)
        ))
        .arg("-t")
        .arg(format!(
            "{:.3}",
            derivation.duration_seconds.unwrap_or(5.0).max(0.001)
        ))
        .arg("-i")
        .arg(source_path);
    if let Some(volume) = derivation.volume {
        command.arg("-filter:a").arg(format!("volume={volume:.3}"));
    }
    command.arg("-c:a").arg("libvorbis").arg(target_path);
    run_command(&mut command, "derive audio query")
}

fn derive_video(
    source_path: &Path,
    target_path: &Path,
    derivation: &Derivation,
) -> Result<(), String> {
    let mut command = Command::new("ffmpeg");
    command
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-ss")
        .arg(format!(
            "{:.3}",
            derivation.start_seconds.unwrap_or(0.0).max(0.0)
        ))
        .arg("-t")
        .arg(format!(
            "{:.3}",
            derivation.duration_seconds.unwrap_or(12.0).max(0.001)
        ))
        .arg("-i")
        .arg(source_path);
    if let Some(width) = derivation.scale_width {
        command.arg("-vf").arg(format!("scale={width}:-2"));
    }
    command
        .arg("-c:v")
        .arg("libvpx")
        .arg("-c:a")
        .arg("libvorbis")
        .arg(target_path);
    run_command(&mut command, "derive video query")
}

fn derive_pdf_comment(
    source_path: &Path,
    target_path: &Path,
    derivation: &Derivation,
) -> Result<(), String> {
    let mut bytes = fs::read(source_path)
        .map_err(|error| format!("failed to read {}: {error}", source_path.display()))?;
    bytes.extend_from_slice(b"\n% ");
    bytes.extend_from_slice(
        derivation
            .comment
            .as_deref()
            .unwrap_or("QUALITY QUERY")
            .as_bytes(),
    );
    bytes.extend_from_slice(b"\n");
    fs::write(target_path, bytes)
        .map_err(|error| format!("failed to write {}: {error}", target_path.display()))
}

fn run_command(command: &mut Command, label: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("{label} failed to start: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("{label} failed with status {}", output.status)
        } else {
            format!("{label} failed: {stderr}")
        })
    }
}

fn image_format_for_path(path: &Path) -> Result<ImageFormat, String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => Ok(ImageFormat::Jpeg),
        Some("png") => Ok(ImageFormat::Png),
        Some("webp") => Ok(ImageFormat::WebP),
        Some("bmp") => Ok(ImageFormat::Bmp),
        Some("tif" | "tiff") => Ok(ImageFormat::Tiff),
        other => Err(format!("unsupported image output extension `{other:?}`")),
    }
}

fn draw_text_overlay(image: &mut RgbImage, text: &str, position: &str) {
    let scale = (image.width().min(image.height()) / 160).max(2);
    let padding = 4 * scale;
    let text_width = text.chars().count() as u32 * 6 * scale;
    let text_height = 7 * scale;
    let box_width = text_width + padding * 2;
    let box_height = text_height + padding * 2;
    let margin = 4 * scale;
    let x = match position {
        "bottom_left" | "top_left" => margin,
        _ => image.width().saturating_sub(box_width + margin),
    };
    let y = match position {
        "top_left" | "top_right" => margin,
        _ => image.height().saturating_sub(box_height + margin),
    };

    fill_rect(image, x, y, box_width, box_height, Rgb([0, 0, 0]));
    let mut cursor = x + padding;
    for ch in text.chars() {
        draw_char(image, ch, cursor, y + padding, scale, Rgb([255, 255, 255]));
        cursor += 6 * scale;
    }
}

fn fill_rect(image: &mut RgbImage, x: u32, y: u32, width: u32, height: u32, color: Rgb<u8>) {
    for yy in y..y.saturating_add(height).min(image.height()) {
        for xx in x..x.saturating_add(width).min(image.width()) {
            image.put_pixel(xx, yy, color);
        }
    }
}

fn draw_char(image: &mut RgbImage, ch: char, x: u32, y: u32, scale: u32, color: Rgb<u8>) {
    let bitmap = char_bitmap(ch.to_ascii_uppercase());
    for (row, pattern) in bitmap.iter().enumerate() {
        for (col, value) in pattern.chars().enumerate() {
            if value != '1' {
                continue;
            }
            fill_rect(
                image,
                x + col as u32 * scale,
                y + row as u32 * scale,
                scale,
                scale,
                color,
            );
        }
    }
}

fn char_bitmap(ch: char) -> [&'static str; 7] {
    match ch {
        'A' => [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'D' => [
            "11110", "10001", "10001", "10001", "10001", "10001", "11110",
        ],
        'E' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "11111",
        ],
        'I' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "11111",
        ],
        'L' => [
            "10000", "10000", "10000", "10000", "10000", "10000", "11111",
        ],
        'Q' => [
            "01110", "10001", "10001", "10001", "10101", "10010", "01101",
        ],
        'R' => [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ],
        'T' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "00100",
        ],
        'U' => [
            "10001", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'Y' => [
            "10001", "10001", "01010", "00100", "00100", "00100", "00100",
        ],
        ' ' => [
            "00000", "00000", "00000", "00000", "00000", "00000", "00000",
        ],
        _ => [
            "11111", "00001", "00010", "00100", "01000", "10000", "11111",
        ],
    }
}

fn ensure_materialized(manifest: &Manifest, output_dir: &Path) -> Result<(), String> {
    for asset in &manifest.assets {
        let path = output_dir.join(&asset.filename);
        if !path.is_file() {
            return Err(format!(
                "quality asset `{}` is missing at {}; run `bun run quality:download` first",
                asset.id,
                path.display()
            ));
        }
    }
    Ok(())
}

fn write_attribution(manifest: &Manifest, output_dir: &Path) -> Result<(), String> {
    let mut file = fs::File::create(output_dir.join("ATTRIBUTION.md"))
        .map_err(|error| format!("failed to write attribution: {error}"))?;
    writeln!(file, "# Attribution\n").map_err(|error| error.to_string())?;
    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.role == "source")
    {
        writeln!(
            file,
            "- **{}**: {}; {}; {}",
            asset.title,
            asset
                .attribution
                .as_deref()
                .unwrap_or("unknown attribution"),
            asset.license.as_deref().unwrap_or("unknown license"),
            asset.page_url.as_deref().unwrap_or("")
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn evaluate_corpus(
    repo_root: &Path,
    manifest: &Manifest,
    output_dir: &Path,
    options: &CliOptions,
) -> Result<QualityReport, String> {
    let run_id = Uuid::new_v4().to_string();
    let collection = options
        .collection
        .clone()
        .unwrap_or_else(|| format!("quality-{run_id}"));
    let run_dir = output_dir.join(".quality-run").join(&run_id);
    fs::create_dir_all(&run_dir)
        .map_err(|error| format!("failed to create {}: {error}", run_dir.display()))?;

    let result =
        evaluate_corpus_with_run_dir(repo_root, manifest, output_dir, &run_dir, &collection);
    match &result {
        Ok(report) if !options.keep_artifacts => {
            cleanup_qdrant_collection(&collection, report).map_err(|error| {
                format!("quality evaluation passed but cleanup failed: {error}")
            })?;
            fs::remove_dir_all(&run_dir).map_err(|error| {
                format!(
                    "quality evaluation passed but failed to remove {}: {error}",
                    run_dir.display()
                )
            })?;
        }
        _ => {}
    }
    result
}

fn evaluate_corpus_with_run_dir(
    _repo_root: &Path,
    manifest: &Manifest,
    output_dir: &Path,
    run_dir: &Path,
    collection: &str,
) -> Result<QualityReport, String> {
    let mut settings = Settings::from_env()?;
    settings.source_image_dir = output_dir.join("sources");
    settings.image_sources = vec![settings.source_image_dir.to_string_lossy().to_string()];
    settings.media_sources_file = run_dir.join("media-sources.txt");
    settings.thumbnail_dir = run_dir.join("thumbnails");
    settings.upload_dir = run_dir.join("uploads");
    settings.indexing_ledger_file = run_dir.join("indexing-ledger.json");
    settings.processing_workflows_file = run_dir.join("processing-workflows.json");
    settings.voice_registry_path = run_dir.join("recognized-voices.json");
    settings.smart_albums_file = run_dir.join("smart-albums.json");
    settings.qdrant_collection = collection.to_string();
    settings.qdrant_request_timeout_ms = settings.qdrant_request_timeout_ms.max(120_000);
    settings.default_search_limit = settings.default_search_limit.max(12);
    settings.audio_transcription_enabled = true;
    settings.ocr_enabled = true;
    settings.video_max_frames = settings.video_max_frames.or(Some(40));
    settings.image_extensions = parse_extensions(".jpg,.jpeg,.png,.webp,.bmp,.tif,.tiff,.gif")?;
    settings.audio_extensions = parse_extensions(".mp3,.wav,.flac,.m4a,.aac,.ogg,.opus")?;
    settings.pdf_extensions = parse_extensions(".pdf")?;

    fs::create_dir_all(&settings.thumbnail_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&settings.upload_dir).map_err(|error| error.to_string())?;

    let tool_statuses = required_tool_statuses();
    let missing_tools = tool_statuses
        .iter()
        .filter(|status| !status.available)
        .map(|status| {
            format!(
                "{}: {}",
                status.name,
                status.detail.clone().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    if !missing_tools.is_empty() {
        return Err(format!(
            "quality evaluation requires media tools:\n{}",
            missing_tools.join("\n")
        ));
    }

    let model_statuses = vec![
        model_status(ModelRole::VisualEmbedding, &settings),
        model_status(ModelRole::FaceDetection, &settings),
        model_status(ModelRole::FaceEmbedding, &settings),
        model_status(ModelRole::AudioTranscription, &settings),
    ];
    let inactive = model_statuses
        .iter()
        .filter(|status| !status.active)
        .map(|status| {
            format!(
                "{} model `{}` is not active: {}",
                status.label,
                status.configured,
                status.detail.clone().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    if !inactive.is_empty() {
        return Err(format!(
            "quality evaluation requires active visual, face detection, face embedding, and audio transcription models:\n{}",
            inactive.join("\n")
        ));
    }

    let source_lookup = source_asset_lookup(manifest, output_dir)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(async {
        let state = Arc::new(AppState::new(settings.clone()));
        let indexer = ImageIndexer::new(
            settings.clone(),
            state.store.clone(),
            state.embedder.clone(),
        );
        let index_response = indexer.index_sources().await;
        if index_response.failed > 0 {
            return Err(format!(
                "quality indexing failed with {} failure(s): {:?}",
                index_response.failed, index_response.errors
            ));
        }

        let points = state.store.scroll_media_points().await?;
        assert_sources_indexed(manifest, &source_lookup, &points)?;
        let service = ImageSearchService::new(
            settings.clone(),
            state.store.clone(),
            state.embedder.clone(),
        );
        let assets_by_id = manifest
            .assets
            .iter()
            .map(|asset| (asset.id.as_str(), asset))
            .collect::<BTreeMap<_, _>>();
        let mut evaluations = Vec::new();
        for search in &manifest.searches {
            let evaluation = evaluate_search_case(
                search,
                &assets_by_id,
                output_dir,
                &settings,
                &service,
                &source_lookup,
            )
            .await;
            evaluations.push(evaluation);
        }
        let report = QualityReport {
            corpus: manifest.name.clone(),
            description: manifest.description.clone(),
            collection: settings.qdrant_collection.clone(),
            index_response: Some(index_response),
            searches: evaluations,
            regression_metrics: regression_metrics(manifest, &model_statuses, &tool_statuses, &[]),
            model_statuses,
            tool_statuses,
        };
        Ok(finalize_report_metrics(report))
    })
}

fn source_asset_lookup(
    manifest: &Manifest,
    output_dir: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let mut lookup = BTreeMap::new();
    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.role == "source")
    {
        let path = output_dir.join(&asset.filename);
        let resolved = path.canonicalize().map_err(|error| {
            format!(
                "failed to resolve source asset `{}` at {}: {error}",
                asset.id,
                path.display()
            )
        })?;
        lookup.insert(resolved.to_string_lossy().to_string(), asset.id.clone());
    }
    Ok(lookup)
}

fn assert_sources_indexed(
    manifest: &Manifest,
    source_lookup: &BTreeMap<String, String>,
    points: &[StoredPoint],
) -> Result<(), String> {
    let mut indexed_source_assets = BTreeSet::new();
    for point in points {
        let Some(payload) = &point.payload else {
            continue;
        };
        let payload: ImagePayload =
            serde_json::from_value(payload.clone()).map_err(|error| error.to_string())?;
        if let Some(asset_id) = payload
            .source_item_uri
            .as_deref()
            .and_then(|uri| source_lookup.get(uri))
        {
            indexed_source_assets.insert(asset_id.clone());
        }
    }
    let missing = manifest
        .assets
        .iter()
        .filter(|asset| asset.role == "source")
        .filter(|asset| !indexed_source_assets.contains(&asset.id))
        .map(|asset| asset.id.clone())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "quality indexing produced no media points for source asset(s): {}",
            missing.join(", ")
        ))
    }
}

async fn evaluate_search_case(
    search: &SearchCase,
    assets_by_id: &BTreeMap<&str, &Asset>,
    output_dir: &Path,
    settings: &Settings,
    service: &ImageSearchService,
    source_lookup: &BTreeMap<String, String>,
) -> SearchEvaluation {
    let mut evaluation = SearchEvaluation {
        id: search.id.clone(),
        capability: search.capability.clone(),
        query_asset: search.query_asset.clone(),
        expected_top_match: search.expected_top_match.clone(),
        top_match: None,
        top_k: Vec::new(),
        expected_non_match_violations: Vec::new(),
        ocr_terms: Vec::new(),
        transcript_terms: Vec::new(),
        passed: false,
        error: None,
    };
    let Some(query_asset) = assets_by_id.get(search.query_asset.as_str()) else {
        evaluation.error = Some("query asset missing from manifest".to_string());
        return evaluation;
    };
    let query_path = output_dir.join(&query_asset.filename);
    match search_query_asset(query_asset, &query_path, settings, service, source_lookup).await {
        Ok(results) => {
            evaluation.top_k = results;
            evaluation.top_match = evaluation
                .top_k
                .first()
                .and_then(|hit| hit.asset_id.clone());
            evaluation.expected_non_match_violations = expected_non_match_violations(
                &evaluation.top_k,
                &search.expected_top_match,
                &search.expected_non_matches,
            );
            evaluation.ocr_terms = evaluate_text_terms(
                &search.expected_ocr_terms,
                &search.expected_top_match,
                service,
                source_lookup,
            )
            .await;
            evaluation.transcript_terms = evaluate_text_terms(
                &search.expected_transcript_terms,
                &search.expected_top_match,
                service,
                source_lookup,
            )
            .await;
            let top_matches =
                evaluation.top_match.as_deref() == Some(search.expected_top_match.as_str());
            let text_passes = evaluation
                .ocr_terms
                .iter()
                .chain(&evaluation.transcript_terms)
                .all(|term| term.matched_expected_asset);
            evaluation.passed =
                top_matches && evaluation.expected_non_match_violations.is_empty() && text_passes;
            if !evaluation.passed && evaluation.error.is_none() {
                evaluation.error = Some(format!(
                    "expected top match `{}`, got {:?}",
                    search.expected_top_match, evaluation.top_match
                ));
            }
        }
        Err(error) => {
            evaluation.error = Some(error);
        }
    }
    evaluation
}

async fn search_query_asset(
    asset: &Asset,
    path: &Path,
    settings: &Settings,
    service: &ImageSearchService,
    source_lookup: &BTreeMap<String, String>,
) -> Result<Vec<SearchHit>, String> {
    let media = decode_query_media(asset, path, settings)?;
    let mut results = Vec::new();
    for item in &media {
        let response = service
            .search_media_filtered(item, Some(12), None, SearchFilters::default())
            .await?;
        results.extend(response.results);
    }
    Ok(search_hits(deduplicate_results(results), source_lookup))
}

fn decode_query_media(
    asset: &Asset,
    path: &Path,
    settings: &Settings,
) -> Result<Vec<DecodedMedia>, String> {
    match asset.kind.as_str() {
        "static_image" | "animated_gif" => {
            let raw = fs::read(path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            Ok(vec![load_media_bytes(&raw, settings)?])
        }
        "audio" => decode_audio_segments(path, settings)
            .map(|segments| segments.into_iter().map(|segment| segment.media).collect()),
        "video" => decode_video_scenes(path, settings)
            .map(|scenes| scenes.into_iter().map(|scene| scene.media).collect()),
        "pdf" => {
            let pdf = decode_pdf(path, settings)?;
            let mut media = pdf
                .pages
                .into_iter()
                .map(|page| page.media)
                .collect::<Vec<_>>();
            media.push(pdf.document_media);
            Ok(media)
        }
        other => Err(format!("unsupported query media kind `{other}`")),
    }
}

fn deduplicate_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut by_media_id = BTreeMap::<String, SearchResult>::new();
    for result in results {
        by_media_id
            .entry(result.image.id.clone())
            .and_modify(|existing| {
                if result.relevance_score.unwrap_or(result.vector_score)
                    > existing.relevance_score.unwrap_or(existing.vector_score)
                {
                    *existing = result.clone();
                }
            })
            .or_insert(result);
    }
    let mut deduped = by_media_id.into_values().collect::<Vec<_>>();
    deduped.sort_by(|left, right| {
        right
            .relevance_score
            .unwrap_or(right.vector_score)
            .total_cmp(&left.relevance_score.unwrap_or(left.vector_score))
    });
    deduped.truncate(12);
    deduped
}

fn search_hits(
    results: Vec<SearchResult>,
    source_lookup: &BTreeMap<String, String>,
) -> Vec<SearchHit> {
    results
        .into_iter()
        .map(|result| SearchHit {
            asset_id: result
                .image
                .source_item_uri
                .as_deref()
                .and_then(|uri| source_lookup.get(uri))
                .cloned(),
            media_id: result.image.id,
            filename: result.image.filename,
            media_kind: result.image.media_kind,
            vector_score: result.vector_score,
            relevance_score: result.relevance_score,
            hash_distance: result.hash_distance,
            near_duplicate: result.near_duplicate,
        })
        .collect()
}

async fn evaluate_text_terms(
    terms: &[String],
    expected_asset_id: &str,
    service: &ImageSearchService,
    source_lookup: &BTreeMap<String, String>,
) -> Vec<TextTermEvaluation> {
    let mut evaluations = Vec::new();
    for term in terms {
        let top_k = match service
            .search_text_filtered(Some(12), term, SearchFilters::default())
            .await
        {
            Ok(response) => search_hits(response.results, source_lookup),
            Err(_) => Vec::new(),
        };
        evaluations.push(TextTermEvaluation {
            term: term.clone(),
            matched_expected_asset: top_k
                .iter()
                .any(|hit| hit.asset_id.as_deref() == Some(expected_asset_id)),
            top_k,
        });
    }
    evaluations
}

fn expected_non_match_violations(
    hits: &[SearchHit],
    expected_top_match: &str,
    expected_non_matches: &[String],
) -> Vec<String> {
    let expected_rank = hits
        .iter()
        .position(|hit| hit.asset_id.as_deref() == Some(expected_top_match))
        .unwrap_or(usize::MAX);
    expected_non_matches
        .iter()
        .filter_map(|asset_id| {
            let rank = hits
                .iter()
                .position(|hit| hit.asset_id.as_deref() == Some(asset_id.as_str()))?;
            (rank < expected_rank).then(|| asset_id.clone())
        })
        .collect()
}

fn required_tool_statuses() -> Vec<ToolStatus> {
    [
        "ffmpeg",
        "ffprobe",
        "pdfinfo",
        "pdftoppm",
        "pdftotext",
        "tesseract",
    ]
    .into_iter()
    .map(tool_status)
    .collect()
}

fn tool_status(name: &str) -> ToolStatus {
    let output = Command::new(name).arg(tool_version_arg(name)).output();
    match output {
        Ok(output) if output.status.success() => ToolStatus {
            name: name.to_string(),
            available: true,
            detail: None,
        },
        Ok(output) => ToolStatus {
            name: name.to_string(),
            available: false,
            detail: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(error) => ToolStatus {
            name: name.to_string(),
            available: false,
            detail: Some(error.to_string()),
        },
    }
}

fn tool_version_arg(name: &str) -> &'static str {
    match name {
        "pdfinfo" | "pdftoppm" | "pdftotext" => "-v",
        "tesseract" => "--version",
        _ => "-version",
    }
}

fn cleanup_qdrant_collection(collection: &str, report: &QualityReport) -> Result<(), String> {
    let settings = Settings::from_env()?;
    let client = reqwest::blocking::Client::new();
    let mut last_error = None;
    for base_url in settings
        .qdrant_url
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let url = format!(
            "{}/collections/{collection}",
            base_url.trim_end_matches('/')
        );
        match client.delete(&url).send() {
            Ok(response) if response.status().is_success() || response.status().as_u16() == 404 => {
                return Ok(());
            }
            Ok(response) => {
                last_error = Some(format!("{url}: HTTP {}", response.status()));
            }
            Err(error) => {
                last_error = Some(format!("{url}: {error}"));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        format!(
            "no Qdrant URL configured while cleaning collection {} from report {}",
            collection, report.collection
        )
    }))
}

fn write_report(repo_root: &Path, report: &QualityReport) -> Result<(), String> {
    let output_dir = repo_root.join("benchmarks/results");
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("failed to create {}: {error}", output_dir.display()))?;
    let json = serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?;
    fs::write(output_dir.join("quality-corpus-report.json"), json)
        .map_err(|error| error.to_string())?;
    let mut markdown = String::new();
    markdown.push_str("# Quality Corpus Report\n\n");
    markdown.push_str(&format!("Corpus: `{}`\n\n", report.corpus));
    markdown.push_str(&format!("Collection: `{}`\n\n", report.collection));
    markdown.push_str("## Searches\n\n");
    for search in &report.searches {
        markdown.push_str(&format!(
            "- `{}`: {}; expected `{}`, got {:?}; passed `{}`\n",
            search.id,
            search.capability,
            search.expected_top_match,
            search.top_match,
            search.passed
        ));
        if !search.expected_non_match_violations.is_empty() {
            markdown.push_str(&format!(
                "  - Non-match violations: {:?}\n",
                search.expected_non_match_violations
            ));
        }
        for term in search.ocr_terms.iter().chain(&search.transcript_terms) {
            markdown.push_str(&format!(
                "  - Text `{}` matched expected asset: `{}`\n",
                term.term, term.matched_expected_asset
            ));
        }
        if let Some(error) = &search.error {
            markdown.push_str(&format!("  - Error: {error}\n"));
        }
    }
    markdown.push_str("\n## Regression Metrics\n\n");
    markdown.push_str(&format!(
        "- Search cases: {}\n",
        report.regression_metrics.search_cases
    ));
    markdown.push_str(&format!(
        "- Passed search cases: {}\n",
        report.regression_metrics.passed_search_cases
    ));
    markdown.push_str(&format!(
        "- Failed search cases: {}\n",
        report.regression_metrics.failed_search_cases
    ));
    markdown.push_str(&format!(
        "- Expected non-match violations: {}\n",
        report.regression_metrics.expected_non_match_violations
    ));
    markdown.push_str(&format!(
        "- Text term failures: {}\n",
        report.regression_metrics.text_term_failures
    ));
    markdown.push_str(&format!(
        "- Degraded mode count: {}\n",
        report.regression_metrics.degraded_mode_count
    ));
    markdown.push_str(&format!(
        "- Inactive model roles: {:?}\n",
        report.regression_metrics.inactive_model_roles
    ));
    fs::write(output_dir.join("quality-corpus-report.md"), markdown)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn finalize_report_metrics(mut report: QualityReport) -> QualityReport {
    report.regression_metrics = regression_metrics(
        &Manifest {
            version: 1,
            name: report.corpus.clone(),
            description: report.description.clone(),
            default_output_dir: String::new(),
            assets: Vec::new(),
            searches: Vec::new(),
        },
        &report.model_statuses,
        &report.tool_statuses,
        &report.searches,
    );
    report
}

fn regression_metrics(
    manifest: &Manifest,
    statuses: &[image_similarity_service::workers::media::models::ModelRuntimeStatus],
    _tool_statuses: &[ToolStatus],
    evaluations: &[SearchEvaluation],
) -> QualityRegressionMetrics {
    let inactive_model_roles = statuses
        .iter()
        .filter(|status| !status.active)
        .map(|status| status.role.clone())
        .collect::<Vec<_>>();
    let search_cases = if evaluations.is_empty() {
        manifest.searches.len()
    } else {
        evaluations.len()
    };
    let passed_search_cases = evaluations
        .iter()
        .filter(|evaluation| evaluation.passed)
        .count();
    let text_term_assertions = evaluations
        .iter()
        .map(|evaluation| evaluation.ocr_terms.len() + evaluation.transcript_terms.len())
        .sum();
    let text_term_failures = evaluations
        .iter()
        .flat_map(|evaluation| {
            evaluation
                .ocr_terms
                .iter()
                .chain(&evaluation.transcript_terms)
        })
        .filter(|term| !term.matched_expected_asset)
        .count();
    QualityRegressionMetrics {
        search_cases,
        passed_search_cases,
        failed_search_cases: evaluations.len().saturating_sub(passed_search_cases),
        expected_top_k_assertions: evaluations
            .iter()
            .map(|evaluation| evaluation.top_k.len())
            .sum(),
        expected_non_match_assertions: evaluations
            .iter()
            .map(|evaluation| evaluation.expected_non_match_violations.len())
            .sum(),
        expected_non_match_violations: evaluations
            .iter()
            .map(|evaluation| evaluation.expected_non_match_violations.len() as u32)
            .sum(),
        text_term_assertions,
        text_term_failures,
        degraded_mode_count: inactive_model_roles.len() as u32,
        inactive_model_roles,
    }
}

fn print_help() {
    println!(
        "Usage: cargo run --manifest-path backend/Cargo.toml --bin quality_corpus -- <check|download|download-models|evaluate> [--output PATH] [--keep-artifacts] [--collection NAME] [--skip-download-check]"
    );
}

#[cfg(test)]
mod tests {
    use super::{
        derive_overlay_text, load_manifest, parse_options, tool_version_arg, validate_manifest,
        CliOptions, Derivation,
    };
    use image::{ImageBuffer, Rgb};
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn parses_quality_corpus_options() {
        let args = vec![
            "--output".to_string(),
            "tmp/quality".to_string(),
            "--keep-artifacts".to_string(),
            "--collection".to_string(),
            "quality-test".to_string(),
            "--skip-download-check".to_string(),
        ];

        let options = parse_options(&args).unwrap();

        assert_eq!(
            options,
            CliOptions {
                output: Some(PathBuf::from("tmp/quality")),
                keep_artifacts: true,
                collection: Some("quality-test".to_string()),
                skip_download_check: true,
            }
        );
    }

    #[test]
    fn validates_checked_in_manifest() {
        let repo_root = super::repo_root().unwrap();
        let manifest =
            load_manifest(&repo_root.join("tests/fixtures/quality-corpus/manifest.json")).unwrap();

        validate_manifest(&manifest).unwrap();
    }

    #[test]
    fn uses_supported_version_flags_for_required_tools() {
        assert_eq!(tool_version_arg("ffmpeg"), "-version");
        assert_eq!(tool_version_arg("ffprobe"), "-version");
        assert_eq!(tool_version_arg("pdfinfo"), "-v");
        assert_eq!(tool_version_arg("pdftoppm"), "-v");
        assert_eq!(tool_version_arg("pdftotext"), "-v");
        assert_eq!(tool_version_arg("tesseract"), "--version");
    }

    #[test]
    fn derived_static_image_changes_bytes_and_stays_decodable() {
        let root = std::env::temp_dir().join(format!("quality-corpus-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.png");
        let target = root.join("target.jpg");
        let image = ImageBuffer::from_pixel(96, 64, Rgb([120_u8, 160, 200]));
        image.save(&source).unwrap();

        derive_overlay_text(
            &source,
            &target,
            &Derivation {
                kind: "overlay_text".to_string(),
                text: Some("QUALITY QUERY".to_string()),
                position: Some("bottom_right".to_string()),
                start_seconds: None,
                duration_seconds: None,
                volume: None,
                scale_width: None,
                comment: None,
            },
        )
        .unwrap();

        assert_ne!(
            std::fs::read(&source).unwrap(),
            std::fs::read(&target).unwrap()
        );
        assert!(image::open(&target).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use image_similarity_service::config::Settings;
use image_similarity_service::workers::media::models::{model_status, ModelRole};
use serde::{Deserialize, Serialize};

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
    expected_top_k: Option<Vec<String>>,
    expected_non_matches: Option<Vec<String>>,
    capability: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
struct SearchCase {
    id: String,
    query_asset: String,
    expected_identity: String,
    expected_top_k: Vec<String>,
    expected_non_matches: Vec<String>,
    capability: String,
}

#[derive(Serialize)]
struct QualityReport {
    corpus: String,
    description: String,
    searches: Vec<SearchCase>,
    model_statuses: Vec<image_similarity_service::workers::media::models::ModelRuntimeStatus>,
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
            let output_dir = output_dir(&repo_root, &manifest, &args)?;
            materialize_corpus(&manifest, &output_dir)?;
            println!(
                "quality corpus `{}` ready at {}",
                manifest.name,
                output_dir.display()
            );
        }
        "evaluate" => {
            let output_dir = output_dir(&repo_root, &manifest, &args)?;
            ensure_materialized(&manifest, &output_dir)?;
            let settings = Settings::default();
            let statuses = vec![
                model_status(ModelRole::VisualEmbedding, &settings),
                model_status(ModelRole::FaceDetection, &settings),
                model_status(ModelRole::FaceEmbedding, &settings),
            ];
            let inactive = statuses
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
                    "quality evaluation requires active visual, face detection, and face embedding models:\n{}",
                    inactive.join("\n")
                ));
            }
            write_report(&repo_root, &manifest, statuses)?;
            println!(
                "quality corpus `{}` evaluation report written",
                manifest.name
            );
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

fn repo_root() -> Result<PathBuf, String> {
    let backend_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    backend_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("could not resolve repo root from {}", backend_dir.display()))
}

fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn output_dir(repo_root: &Path, manifest: &Manifest, args: &[String]) -> Result<PathBuf, String> {
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--output requires a path".to_string());
                };
                output = Some(PathBuf::from(value));
                index += 2;
            }
            unknown => return Err(format!("unknown option `{unknown}`")),
        }
    }

    let output = output.unwrap_or_else(|| PathBuf::from(&manifest.default_output_dir));
    if output.is_absolute() {
        Ok(output)
    } else {
        Ok(repo_root.join(output))
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
    let mut identities = BTreeMap::<String, u32>::new();
    for asset in &manifest.assets {
        require_field("asset id", &asset.id)?;
        require_field("asset filename", &asset.filename)?;
        require_field("asset title", &asset.title)?;
        require_field("asset identity", &asset.identity)?;
        if !ids.insert(asset.id.as_str()) {
            return Err(format!("duplicate asset id `{}`", asset.id));
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
                require_https("download_url", asset.download_url.as_deref(), &asset.id)?;
                require_https("page_url", asset.page_url.as_deref(), &asset.id)?;
                require_field("license", asset.license.as_deref().unwrap_or_default())?;
                require_field(
                    "attribution",
                    asset.attribution.as_deref().unwrap_or_default(),
                )?;
                *identities.entry(asset.identity.clone()).or_default() += 1;
            }
            "query" => {
                if asset.copy_of.is_none() {
                    return Err(format!("query asset `{}` needs copy_of", asset.id));
                }
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
        if asset.kind != "static_image" {
            return Err(format!("asset `{}` must be a static_image", asset.id));
        }
    }
    for asset in &manifest.assets {
        if let Some(copy_of) = asset.copy_of.as_deref() {
            if !source_ids.contains(copy_of) {
                return Err(format!(
                    "asset `{}` copies unknown source `{copy_of}`",
                    asset.id
                ));
            }
        }
    }
    if identities.values().any(|count| *count < 2) {
        return Err("each identity needs at least two source assets".to_string());
    }
    for search in &manifest.searches {
        require_field("search id", &search.id)?;
        require_field("search capability", &search.capability)?;
        require_field("search expected identity", &search.expected_identity)?;
        if !ids.contains(search.query_asset.as_str()) {
            return Err(format!("search `{}` references missing query", search.id));
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
        fs::copy(&source_path, &target_path).map_err(|error| {
            format!(
                "failed to copy query asset `{}` from {} to {}: {error}",
                asset.id,
                source_path.display(),
                target_path.display()
            )
        })?;
    }
    write_attribution(manifest, output_dir)?;
    Ok(())
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

fn write_report(
    repo_root: &Path,
    manifest: &Manifest,
    statuses: Vec<image_similarity_service::workers::media::models::ModelRuntimeStatus>,
) -> Result<(), String> {
    let output_dir = repo_root.join("benchmarks/results");
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("failed to create {}: {error}", output_dir.display()))?;
    let report = QualityReport {
        corpus: manifest.name.clone(),
        description: manifest.description.clone(),
        searches: manifest.searches.clone(),
        model_statuses: statuses,
    };
    let json = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    fs::write(output_dir.join("quality-corpus-report.json"), json)
        .map_err(|error| error.to_string())?;
    let mut markdown = String::new();
    markdown.push_str("# Quality Corpus Report\n\n");
    markdown.push_str(&format!("Corpus: `{}`\n\n", manifest.name));
    markdown.push_str("## Searches\n\n");
    for search in &manifest.searches {
        markdown.push_str(&format!(
            "- `{}`: expected identity `{}`; top-k {:?}\n",
            search.id, search.expected_identity, search.expected_top_k
        ));
    }
    fs::write(output_dir.join("quality-corpus-report.md"), markdown)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn print_help() {
    println!(
        "Usage: cargo run --manifest-path backend/Cargo.toml --bin quality_corpus -- <check|download|evaluate> [--output PATH]"
    );
}

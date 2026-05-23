use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
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
    download_url: Option<String>,
    page_url: Option<String>,
    license: String,
    attribution: String,
    copy_of: Option<String>,
}

#[derive(Deserialize)]
struct SearchCase {
    id: String,
    query_asset: String,
    expected_top_match: String,
    capability: String,
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
    let manifest_path = repo_root.join("tests/fixtures/sample-corpus/manifest.json");
    let manifest = load_manifest(&manifest_path)?;
    validate_manifest(&manifest)?;

    match command.as_str() {
        "check" => {
            println!(
                "sample corpus `{}` is valid: {} assets, {} searches",
                manifest.name,
                manifest.assets.len(),
                manifest.searches.len()
            );
        }
        "download" | "showcase" => {
            let output_dir = output_dir(&repo_root, &manifest, &args)?;
            materialize_corpus(&manifest, &output_dir)?;
            println!(
                "sample corpus `{}` ready at {}",
                manifest.name,
                output_dir.display()
            );
        }
        "help" | "--help" | "-h" => {
            print_help();
        }
        other => {
            return Err(format!(
                "unknown command `{other}`\n\nRun `cargo run --manifest-path backend/Cargo.toml --bin sample_corpus -- help` for usage."
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
            "unsupported sample corpus version {}",
            manifest.version
        ));
    }
    if manifest.assets.is_empty() {
        return Err("sample corpus needs at least one asset".to_string());
    }
    if manifest.searches.is_empty() {
        return Err("sample corpus needs at least one search case".to_string());
    }

    let mut ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    for asset in &manifest.assets {
        require_field("asset id", &asset.id)?;
        require_field("asset filename", &asset.filename)?;
        require_field("asset title", &asset.title)?;
        require_field("asset kind", &asset.kind)?;
        require_field("asset role", &asset.role)?;
        require_field("asset license", &asset.license)?;
        require_field("asset attribution", &asset.attribution)?;
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
            }
            "query" => {
                if asset.copy_of.is_none() {
                    return Err(format!("query asset `{}` needs copy_of", asset.id));
                }
            }
            other => {
                return Err(format!(
                    "asset `{}` has unsupported role `{other}`",
                    asset.id
                ))
            }
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

    for required_kind in ["static_image", "animated_gif", "audio", "video", "pdf"] {
        if !manifest
            .assets
            .iter()
            .any(|asset| asset.kind == required_kind)
        {
            return Err(format!("missing `{required_kind}` sample asset"));
        }
    }

    for search in &manifest.searches {
        require_field("search id", &search.id)?;
        require_field("search capability", &search.capability)?;
        if !ids.contains(search.query_asset.as_str()) {
            return Err(format!(
                "search `{}` references missing query `{}`",
                search.id, search.query_asset
            ));
        }
        if !ids.contains(search.expected_top_match.as_str()) {
            return Err(format!(
                "search `{}` references missing expected match `{}`",
                search.id, search.expected_top_match
            ));
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
        .user_agent("image-similarity-service-sample-corpus/0.1")
        .build()
        .map_err(|error| format!("failed to build HTTP client: {error}"))?;

    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.role == "source")
    {
        let target = output_dir.join(&asset.filename);
        if target.exists() {
            println!("exists {}", target.display());
            continue;
        }
        let Some(url) = asset.download_url.as_deref() else {
            return Err(format!(
                "source asset `{}` is missing download_url",
                asset.id
            ));
        };
        download_file(&client, url, &target)?;
    }

    for asset in manifest.assets.iter().filter(|asset| asset.role == "query") {
        let Some(source_id) = asset.copy_of.as_deref() else {
            return Err(format!("query asset `{}` is missing copy_of", asset.id));
        };
        let Some(source) = assets_by_id.get(source_id) else {
            return Err(format!(
                "query asset `{}` copies unknown `{source_id}`",
                asset.id
            ));
        };
        let source_path = output_dir.join(&source.filename);
        let target = output_dir.join(&asset.filename);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::copy(&source_path, &target).map_err(|error| {
            format!(
                "failed to copy {} to {}: {error}",
                source_path.display(),
                target.display()
            )
        })?;
        println!("copied {}", target.display());
    }

    write_attribution(manifest, output_dir)
}

fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    target: &Path,
) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    println!("downloading {url}");
    let mut response = client
        .get(url)
        .send()
        .map_err(|error| format!("GET {url} failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GET {url} failed: {error}"))?;
    let mut file = fs::File::create(target)
        .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
    response
        .copy_to(&mut file)
        .map_err(|error| format!("failed to write {}: {error}", target.display()))?;
    println!("saved {}", target.display());
    Ok(())
}

fn write_attribution(manifest: &Manifest, output_dir: &Path) -> Result<(), String> {
    let path = output_dir.join("ATTRIBUTION.md");
    let mut file = fs::File::create(&path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    writeln!(file, "# Sample Corpus Attribution").map_err(|error| error.to_string())?;
    writeln!(file).map_err(|error| error.to_string())?;
    writeln!(file, "{}", manifest.description).map_err(|error| error.to_string())?;
    writeln!(file).map_err(|error| error.to_string())?;

    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.role == "source")
    {
        writeln!(file, "## {}", asset.title).map_err(|error| error.to_string())?;
        writeln!(file).map_err(|error| error.to_string())?;
        writeln!(file, "- File: `{}`", asset.filename).map_err(|error| error.to_string())?;
        writeln!(file, "- Kind: `{}`", asset.kind).map_err(|error| error.to_string())?;
        writeln!(file, "- Attribution: {}", asset.attribution)
            .map_err(|error| error.to_string())?;
        writeln!(file, "- License: {}", asset.license).map_err(|error| error.to_string())?;
        if let Some(page_url) = asset.page_url.as_deref() {
            writeln!(file, "- Source page: {page_url}").map_err(|error| error.to_string())?;
        }
        if let Some(download_url) = asset.download_url.as_deref() {
            writeln!(file, "- Download URL: {download_url}").map_err(|error| error.to_string())?;
        }
        writeln!(file).map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn print_help() {
    println!(
        "Usage:
  cargo run --manifest-path backend/Cargo.toml --bin sample_corpus -- check
  cargo run --manifest-path backend/Cargo.toml --bin sample_corpus -- download [--output PATH]

Commands:
  check      Validate tests/fixtures/sample-corpus/manifest.json.
  download   Download source media, create query copies, and write ATTRIBUTION.md.
  showcase   Alias for download.
"
    );
}

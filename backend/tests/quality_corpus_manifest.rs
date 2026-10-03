use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

const QUALITY_CORPUS_MANIFEST: &str =
    include_str!("../../tests/fixtures/quality-corpus/manifest.json");

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    default_output_dir: String,
    assets: Vec<Asset>,
    searches: Vec<SearchCase>,
}

#[derive(Deserialize)]
struct Asset {
    id: String,
    kind: String,
    role: String,
    filename: String,
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

#[derive(Deserialize)]
struct Derivation {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct SearchCase {
    id: String,
    query_asset: String,
    expected_identity: String,
    expected_top_match: String,
    expected_top_k: Vec<String>,
    expected_non_matches: Vec<String>,
    expected_ocr_terms: Option<Vec<String>>,
    expected_transcript_terms: Option<Vec<String>>,
    capability: String,
}

#[test]
fn quality_corpus_manifest_defines_public_all_media_quality_cases() {
    let manifest: Manifest = serde_json::from_str(QUALITY_CORPUS_MANIFEST).unwrap();

    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.default_output_dir, "sample-images/quality");
    assert!(!manifest.assets.is_empty());
    assert!(!manifest.searches.is_empty());

    let mut ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    let mut assets_by_id = BTreeMap::new();
    let mut source_kinds = BTreeSet::new();
    let mut query_derivations = BTreeSet::new();
    let mut ocr_cases = 0;
    let mut transcript_cases = 0;

    for asset in &manifest.assets {
        assert!(ids.insert(asset.id.as_str()), "duplicate {}", asset.id);
        assets_by_id.insert(asset.id.as_str(), asset);
        assert!(valid_kind(&asset.kind), "{} unsupported kind", asset.id);
        assert!(
            !asset.filename.starts_with('/'),
            "{} must be relative",
            asset.id
        );
        assert!(
            !asset.filename.contains(".."),
            "{} escapes output dir",
            asset.id
        );
        assert!(
            filename_matches_kind(&asset.filename, &asset.kind),
            "{} filename `{}` does not match kind `{}`",
            asset.id,
            asset.filename,
            asset.kind
        );
        assert!(
            !asset.identity.trim().is_empty(),
            "{} missing identity",
            asset.id
        );

        match asset.role.as_str() {
            "source" => {
                source_ids.insert(asset.id.as_str());
                source_kinds.insert(asset.kind.as_str());
                assert!(asset
                    .download_url
                    .as_deref()
                    .is_some_and(|url| url.starts_with("https://")));
                assert!(asset
                    .page_url
                    .as_deref()
                    .is_some_and(|url| url.starts_with("https://")));
                assert!(asset
                    .license
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()));
                assert!(asset
                    .attribution
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()));
                assert!(
                    asset.derivation.is_none(),
                    "{} source has derivation",
                    asset.id
                );
            }
            "query" => {
                assert!(asset.copy_of.is_some(), "{} query needs copy_of", asset.id);
                let derivation = asset
                    .derivation
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} query needs derivation", asset.id));
                assert!(
                    valid_derivation(&derivation.kind),
                    "{} unsupported derivation `{}`",
                    asset.id,
                    derivation.kind
                );
                query_derivations.insert(derivation.kind.as_str());
                assert!(asset
                    .expected_top_match
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()));
                assert!(asset
                    .expected_top_k
                    .as_ref()
                    .is_some_and(|ids| !ids.is_empty()));
                assert!(asset
                    .expected_non_matches
                    .as_ref()
                    .is_some_and(|ids| !ids.is_empty()));
                assert!(asset
                    .capability
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()));
                if asset
                    .expected_ocr_terms
                    .as_ref()
                    .is_some_and(|terms| !terms.is_empty())
                {
                    ocr_cases += 1;
                }
                if asset
                    .expected_transcript_terms
                    .as_ref()
                    .is_some_and(|terms| !terms.is_empty())
                {
                    transcript_cases += 1;
                }
            }
            other => panic!("unsupported asset role {other}"),
        }
    }

    for required_kind in ["static_image", "animated_gif", "audio", "video", "pdf"] {
        assert!(
            source_kinds.contains(required_kind),
            "missing source asset kind {required_kind}"
        );
    }

    for required_derivation in [
        "overlay_text",
        "gif_overlay_text",
        "audio_reencode",
        "video_reencode",
        "append_pdf_comment",
    ] {
        assert!(
            query_derivations.contains(required_derivation),
            "missing query derivation {required_derivation}"
        );
    }
    assert!(ocr_cases > 0, "expected OCR quality cases");
    assert!(
        transcript_cases > 0,
        "expected ASR transcript quality cases"
    );

    for asset in &manifest.assets {
        if let Some(copy_of) = &asset.copy_of {
            let source = assets_by_id
                .get(copy_of.as_str())
                .expect("copy source exists");
            assert_eq!(source.role, "source");
            assert_eq!(source.identity, asset.identity);
            assert_eq!(source.kind, asset.kind);
        }
        for asset_id in asset
            .expected_top_match
            .iter()
            .chain(asset.expected_top_k.iter().flatten())
            .chain(asset.expected_non_matches.iter().flatten())
        {
            assert!(
                ids.contains(asset_id.as_str()),
                "{} -> {asset_id}",
                asset.id
            );
        }
    }

    let mut capabilities = BTreeSet::new();
    for search in &manifest.searches {
        assert!(ids.contains(search.query_asset.as_str()), "{}", search.id);
        assert!(
            ids.contains(search.expected_top_match.as_str()),
            "{}",
            search.id
        );
        assert!(!search.expected_identity.trim().is_empty());
        assert!(!search.expected_top_k.is_empty());
        assert!(!search.expected_non_matches.is_empty());
        assert!(!search.capability.trim().is_empty());
        capabilities.insert(search.capability.as_str());
        for asset_id in search
            .expected_top_k
            .iter()
            .chain(&search.expected_non_matches)
        {
            assert!(
                ids.contains(asset_id.as_str()),
                "{} -> {asset_id}",
                search.id
            );
        }
        if search
            .expected_ocr_terms
            .as_ref()
            .is_some_and(|terms| !terms.is_empty())
        {
            ocr_cases += 1;
        }
        if search
            .expected_transcript_terms
            .as_ref()
            .is_some_and(|terms| !terms.is_empty())
        {
            transcript_cases += 1;
        }
        let query = assets_by_id
            .get(search.query_asset.as_str())
            .expect("query asset");
        assert_eq!(query.role, "query");
        assert_eq!(query.identity, search.expected_identity);
    }
    assert!(capabilities.contains("face person search"));
    assert!(capabilities.contains("static image perturbation search"));
    assert!(capabilities.contains("audio ASR transcript search"));
    assert!(capabilities.contains("video scene and ASR transcript search"));
    assert!(capabilities.contains("PDF page and document search"));
    assert!(ocr_cases > 0);
    assert!(transcript_cases > 0);
    assert!(!source_ids.is_empty());
}

fn valid_kind(kind: &str) -> bool {
    matches!(
        kind,
        "static_image" | "animated_gif" | "audio" | "video" | "pdf"
    )
}

fn valid_derivation(kind: &str) -> bool {
    matches!(
        kind,
        "overlay_text"
            | "gif_overlay_text"
            | "audio_reencode"
            | "video_reencode"
            | "append_pdf_comment"
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

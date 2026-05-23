use std::collections::BTreeSet;

use serde::Deserialize;

const SAMPLE_CORPUS_MANIFEST: &str =
    include_str!("../../tests/fixtures/sample-corpus/manifest.json");

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

#[test]
fn sample_corpus_manifest_defines_supported_media_showcases() {
    let manifest: Manifest = serde_json::from_str(SAMPLE_CORPUS_MANIFEST).unwrap();

    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.default_output_dir, "sample-images/showcase");
    assert!(!manifest.assets.is_empty());
    assert!(!manifest.searches.is_empty());

    let mut ids = BTreeSet::new();
    for asset in &manifest.assets {
        assert!(
            ids.insert(asset.id.as_str()),
            "duplicate asset id {}",
            asset.id
        );
        assert!(
            !asset.filename.starts_with('/'),
            "{} must be relative",
            asset.id
        );
        assert!(
            !asset.license.trim().is_empty(),
            "{} missing license",
            asset.id
        );
        assert!(
            !asset.attribution.trim().is_empty(),
            "{} missing attribution",
            asset.id
        );
        match asset.role.as_str() {
            "source" => {
                assert!(
                    asset
                        .download_url
                        .as_deref()
                        .is_some_and(|url| url.starts_with("https://")),
                    "{} source assets need https download URLs",
                    asset.id
                );
                assert!(
                    asset
                        .page_url
                        .as_deref()
                        .is_some_and(|url| url.starts_with("https://")),
                    "{} source assets need attribution page URLs",
                    asset.id
                );
            }
            "query" => {
                assert!(
                    asset.copy_of.is_some(),
                    "{} query assets should derive from a source asset",
                    asset.id
                );
            }
            other => panic!("unsupported asset role {other}"),
        }
    }

    for asset in &manifest.assets {
        if let Some(copy_of) = &asset.copy_of {
            assert!(
                ids.contains(copy_of.as_str()),
                "{} copies missing {copy_of}",
                asset.id
            );
        }
    }

    for kind in ["static_image", "animated_gif", "audio", "video", "pdf"] {
        assert!(
            manifest.assets.iter().any(|asset| asset.kind == kind),
            "missing sample asset kind {kind}"
        );
    }

    for search in &manifest.searches {
        assert!(
            ids.contains(search.query_asset.as_str()),
            "{} query missing",
            search.id
        );
        assert!(
            ids.contains(search.expected_top_match.as_str()),
            "{} expected match missing",
            search.id
        );
        assert!(
            !search.capability.trim().is_empty(),
            "{} missing capability label",
            search.id
        );
    }
}

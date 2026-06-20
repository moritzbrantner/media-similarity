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
    expected_top_k: Option<Vec<String>>,
    expected_non_matches: Option<Vec<String>>,
    capability: Option<String>,
}

#[derive(Deserialize)]
struct SearchCase {
    id: String,
    query_asset: String,
    expected_identity: String,
    expected_top_k: Vec<String>,
    expected_non_matches: Vec<String>,
    capability: String,
}

#[test]
fn quality_corpus_manifest_defines_public_face_quality_cases() {
    let manifest: Manifest = serde_json::from_str(QUALITY_CORPUS_MANIFEST).unwrap();

    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.default_output_dir, "sample-images/quality");
    assert!(!manifest.assets.is_empty());
    assert!(!manifest.searches.is_empty());

    let mut ids = BTreeSet::new();
    let mut sources_by_identity = BTreeMap::<String, usize>::new();
    let mut assets_by_id = BTreeMap::new();
    for asset in &manifest.assets {
        assert!(ids.insert(asset.id.as_str()), "duplicate {}", asset.id);
        assets_by_id.insert(asset.id.as_str(), asset);
        assert_eq!(asset.kind, "static_image");
        assert!(!asset.filename.starts_with('/'));
        assert!(!asset.filename.contains(".."));
        assert!(!asset.identity.trim().is_empty());
        match asset.role.as_str() {
            "source" => {
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
                    .is_some_and(|value| !value.is_empty()));
                assert!(asset
                    .attribution
                    .as_deref()
                    .is_some_and(|value| !value.is_empty()));
                *sources_by_identity
                    .entry(asset.identity.clone())
                    .or_default() += 1;
            }
            "query" => {
                assert!(asset.copy_of.is_some());
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
                    .is_some_and(|value| !value.is_empty()));
            }
            other => panic!("unsupported asset role {other}"),
        }
    }

    assert!(
        sources_by_identity.values().all(|count| *count >= 2),
        "{sources_by_identity:?}"
    );

    for asset in &manifest.assets {
        if let Some(copy_of) = &asset.copy_of {
            let source = assets_by_id
                .get(copy_of.as_str())
                .expect("copy source exists");
            assert_eq!(source.role, "source");
            assert_eq!(source.identity, asset.identity);
        }
    }

    for search in &manifest.searches {
        assert!(ids.contains(search.query_asset.as_str()), "{}", search.id);
        assert!(!search.expected_identity.trim().is_empty());
        assert_eq!(search.capability, "face person search");
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
        let query = assets_by_id
            .get(search.query_asset.as_str())
            .expect("query asset");
        assert_eq!(query.role, "query");
        assert_eq!(query.identity, search.expected_identity);
    }
}

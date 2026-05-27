use serde::{Deserialize, Serialize};

use super::models::ImagePayload;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SmartAlbum {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub criteria: SmartAlbumCriteria,
    pub sort: AlbumSortMode,
    pub limit: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EditableSmartAlbum {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub criteria: SmartAlbumCriteria,
    #[serde(default)]
    pub sort: AlbumSortMode,
    #[serde(default = "default_album_limit")]
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SmartAlbumCriteria {
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub media_kind: Option<String>,
    #[serde(default)]
    pub name_query: Option<String>,
    #[serde(default)]
    pub camera_query: Option<String>,
    #[serde(default)]
    pub keyword_query: Option<String>,
    #[serde(default)]
    pub text_query: Option<String>,
    #[serde(default)]
    pub person_id: Option<String>,
    #[serde(default)]
    pub speaker_id: Option<String>,
    #[serde(default)]
    pub has_gps: Option<bool>,
    #[serde(default)]
    pub duplicate_status: DuplicateStatusFilter,
    #[serde(default)]
    pub orientation: Option<AlbumOrientationFilter>,
    #[serde(default)]
    pub min_width: Option<u32>,
    #[serde(default)]
    pub max_width: Option<u32>,
    #[serde(default)]
    pub min_height: Option<u32>,
    #[serde(default)]
    pub max_height: Option<u32>,
    #[serde(default)]
    pub min_size_bytes: Option<u64>,
    #[serde(default)]
    pub max_size_bytes: Option<u64>,
    #[serde(default)]
    pub modified_from: Option<f64>,
    #[serde(default)]
    pub modified_to: Option<f64>,
    #[serde(default)]
    pub captured_from: Option<f64>,
    #[serde(default)]
    pub captured_to: Option<f64>,
}

impl Default for SmartAlbumCriteria {
    fn default() -> Self {
        Self {
            source_type: None,
            media_kind: None,
            name_query: None,
            camera_query: None,
            keyword_query: None,
            text_query: None,
            person_id: None,
            speaker_id: None,
            has_gps: None,
            duplicate_status: DuplicateStatusFilter::All,
            orientation: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            min_size_bytes: None,
            max_size_bytes: None,
            modified_from: None,
            modified_to: None,
            captured_from: None,
            captured_to: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateStatusFilter {
    #[default]
    All,
    Only,
    Exclude,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumOrientationFilter {
    Landscape,
    Portrait,
    Square,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumSortMode {
    CapturedNewest,
    Filename,
    #[default]
    ModifiedNewest,
    SizeLargest,
    DuplicateGroupSize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SmartAlbumResultsResponse {
    pub album: SmartAlbum,
    pub count: usize,
    pub total: usize,
    pub offset: usize,
    pub limit: u32,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub duplicate_groups: Vec<DuplicateGroupSummary>,
    pub results: Vec<SmartAlbumResult>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SmartAlbumResult {
    pub image: ImagePayload,
    #[serde(default)]
    pub duplicate_group_id: Option<String>,
    pub duplicate_group_size: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DuplicateGroupSummary {
    pub id: String,
    pub size: usize,
    pub representative_media_id: String,
    pub media_ids: Vec<String>,
}

pub fn default_album_limit() -> u32 {
    60
}

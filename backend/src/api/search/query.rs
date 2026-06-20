use crate::api::ApiError;
use crate::workers::search::{NearDuplicateFilter, OrientationFilter, SearchFilters};

#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub limit: Option<u32>,
    pub ocr_text: Option<String>,
    pub person_id: Option<String>,
    pub source_type: Option<String>,
    pub media_kind: Option<String>,
    pub name_query: Option<String>,
    pub camera_query: Option<String>,
    pub keyword_query: Option<String>,
    pub has_gps: Option<String>,
    pub near_duplicate: Option<String>,
    pub orientation: Option<String>,
    pub min_width: Option<u32>,
    pub max_width: Option<u32>,
    pub min_height: Option<u32>,
    pub max_height: Option<u32>,
    pub min_size_bytes: Option<u64>,
    pub max_size_bytes: Option<u64>,
    pub modified_from: Option<f64>,
    pub modified_to: Option<f64>,
    pub captured_from: Option<f64>,
    pub captured_to: Option<f64>,
}

impl SearchQuery {
    pub fn search_filters(&self) -> Result<SearchFilters, ApiError> {
        Ok(SearchFilters {
            source_type: normalized_filter(self.source_type.as_deref())
                .filter(|value| value != "all"),
            media_kind: normalized_filter(self.media_kind.as_deref())
                .filter(|value| value != "all")
                .map(validate_media_kind)
                .transpose()?,
            name_query: normalized_filter(self.name_query.as_deref()),
            camera_query: normalized_filter(self.camera_query.as_deref()),
            keyword_query: normalized_filter(self.keyword_query.as_deref()),
            has_gps: parse_has_gps(self.has_gps.as_deref())?,
            near_duplicate: parse_near_duplicate(self.near_duplicate.as_deref())?,
            orientation: parse_orientation(self.orientation.as_deref())?,
            min_width: self.min_width,
            max_width: self.max_width,
            min_height: self.min_height,
            max_height: self.max_height,
            min_size_bytes: self.min_size_bytes,
            max_size_bytes: self.max_size_bytes,
            modified_from: validate_optional_seconds("modified_from", self.modified_from)?,
            modified_to: validate_optional_seconds("modified_to", self.modified_to)?,
            captured_from: validate_optional_seconds("captured_from", self.captured_from)?,
            captured_to: validate_optional_seconds("captured_to", self.captured_to)?,
            person_id: normalized_filter(self.person_id.as_deref()),
        })
    }
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn validate_media_kind(value: String) -> Result<String, ApiError> {
    match value.as_str() {
        "static_image" | "animated_gif" | "video_scene" | "audio" | "pdf_page" | "pdf_document" => {
            Ok(value)
        }
        _ => Err(ApiError::bad_request(
            "media_kind must be one of all, static_image, animated_gif, video_scene, audio, pdf_page, pdf_document",
        )),
    }
}

fn parse_has_gps(value: Option<&str>) -> Result<Option<bool>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("yes") => Ok(Some(true)),
        Some("no") => Ok(Some(false)),
        Some(_) => Err(ApiError::bad_request("has_gps must be one of all, yes, no")),
    }
}

fn parse_near_duplicate(value: Option<&str>) -> Result<Option<NearDuplicateFilter>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("only") => Ok(Some(NearDuplicateFilter::Only)),
        Some("exclude") => Ok(Some(NearDuplicateFilter::Exclude)),
        Some(_) => Err(ApiError::bad_request(
            "near_duplicate must be one of all, only, exclude",
        )),
    }
}

fn parse_orientation(value: Option<&str>) -> Result<Option<OrientationFilter>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("landscape") => Ok(Some(OrientationFilter::Landscape)),
        Some("portrait") => Ok(Some(OrientationFilter::Portrait)),
        Some("square") => Ok(Some(OrientationFilter::Square)),
        Some(_) => Err(ApiError::bad_request(
            "orientation must be one of all, landscape, portrait, square",
        )),
    }
}

fn validate_optional_seconds(name: &str, value: Option<f64>) -> Result<Option<f64>, ApiError> {
    match value {
        Some(value) if !value.is_finite() || value < 0.0 => Err(ApiError::bad_request(format!(
            "{name} must be a non-negative Unix timestamp in seconds"
        ))),
        _ => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::SearchQuery;
    use crate::workers::search::{NearDuplicateFilter, OrientationFilter};

    #[test]
    fn search_query_filters_trim_defaults_and_parse_enums() {
        let query = SearchQuery {
            limit: None,
            ocr_text: None,
            person_id: Some(" person-1 ".to_string()),
            source_type: Some(" all ".to_string()),
            media_kind: Some(" static_image ".to_string()),
            name_query: Some(" sunrise ".to_string()),
            camera_query: None,
            keyword_query: Some(" travel ".to_string()),
            has_gps: Some("yes".to_string()),
            near_duplicate: Some("exclude".to_string()),
            orientation: Some("landscape".to_string()),
            min_width: Some(640),
            max_width: Some(1920),
            min_height: None,
            max_height: None,
            min_size_bytes: Some(1024),
            max_size_bytes: None,
            modified_from: Some(1_700_000_000.0),
            modified_to: None,
            captured_from: None,
            captured_to: Some(1_800_000_000.0),
        };

        let filters = query.search_filters().unwrap();
        assert_eq!(filters.person_id.as_deref(), Some("person-1"));
        assert_eq!(filters.source_type, None);
        assert_eq!(filters.media_kind.as_deref(), Some("static_image"));
        assert_eq!(filters.min_width, Some(640));
        assert_eq!(filters.min_size_bytes, Some(1024));
        assert_eq!(filters.modified_from, Some(1_700_000_000.0));
        assert_eq!(filters.captured_to, Some(1_800_000_000.0));
        assert_eq!(filters.has_gps, Some(true));
        assert_eq!(filters.near_duplicate, Some(NearDuplicateFilter::Exclude));
        assert_eq!(filters.orientation, Some(OrientationFilter::Landscape));
    }
}

use std::fs;
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::smart_albums::{
    AlbumOrientationFilter, DuplicateStatusFilter, EditableSmartAlbum, SmartAlbum,
    SmartAlbumCriteria,
};

const FILE_VERSION: u32 = 1;
const MAX_ALBUM_NAME_LENGTH: usize = 80;
const MAX_ALBUM_DESCRIPTION_LENGTH: usize = 500;

#[derive(Debug, Deserialize, Serialize)]
struct SmartAlbumFile {
    version: u32,
    albums: Vec<SmartAlbum>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SmartAlbumListResponse {
    pub albums: Vec<SmartAlbum>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeleteSmartAlbumResponse {
    pub deleted: bool,
}

pub fn list_smart_albums(path: &Path) -> Result<SmartAlbumListResponse, String> {
    Ok(SmartAlbumListResponse {
        albums: sorted_albums(read_album_file(path)?.albums),
    })
}

pub fn create_smart_album(path: &Path, input: EditableSmartAlbum) -> Result<SmartAlbum, String> {
    let input = validate_editable_album(input)?;
    let mut file = read_album_file(path)?;
    let now = timestamp();
    let album = SmartAlbum {
        id: format!("album-{}", Uuid::new_v4()),
        name: input.name,
        description: input.description,
        criteria: input.criteria,
        sort: input.sort,
        limit: input.limit,
        created_at: now.clone(),
        updated_at: now,
    };
    file.albums.push(album.clone());
    write_album_file(path, &file)?;
    Ok(album)
}

pub fn update_smart_album(
    path: &Path,
    album_id: &str,
    input: EditableSmartAlbum,
) -> Result<SmartAlbum, String> {
    let input = validate_editable_album(input)?;
    let mut file = read_album_file(path)?;
    let Some(album) = file.albums.iter_mut().find(|album| album.id == album_id) else {
        return Err(format!("Unknown smart album `{album_id}`"));
    };
    album.name = input.name;
    album.description = input.description;
    album.criteria = input.criteria;
    album.sort = input.sort;
    album.limit = input.limit;
    album.updated_at = timestamp();
    let album = album.clone();
    write_album_file(path, &file)?;
    Ok(album)
}

pub fn delete_smart_album(path: &Path, album_id: &str) -> Result<DeleteSmartAlbumResponse, String> {
    let mut file = read_album_file(path)?;
    let previous_len = file.albums.len();
    file.albums.retain(|album| album.id != album_id);
    if file.albums.len() == previous_len {
        return Err(format!("Unknown smart album `{album_id}`"));
    }
    write_album_file(path, &file)?;
    Ok(DeleteSmartAlbumResponse { deleted: true })
}

pub fn get_smart_album(path: &Path, album_id: &str) -> Result<SmartAlbum, String> {
    read_album_file(path)?
        .albums
        .into_iter()
        .find(|album| album.id == album_id)
        .ok_or_else(|| format!("Unknown smart album `{album_id}`"))
}

pub fn validate_editable_album(
    mut input: EditableSmartAlbum,
) -> Result<EditableSmartAlbum, String> {
    input.name = normalize_required_text("name", input.name, MAX_ALBUM_NAME_LENGTH)?;
    input.description = input
        .description
        .map(|value| normalize_optional_text("description", value, MAX_ALBUM_DESCRIPTION_LENGTH))
        .transpose()?
        .flatten();
    input.criteria = validate_criteria(input.criteria)?;
    if input.limit == 0 || input.limit > 500 {
        return Err("limit must be between 1 and 500".to_string());
    }
    Ok(input)
}

fn validate_criteria(mut criteria: SmartAlbumCriteria) -> Result<SmartAlbumCriteria, String> {
    criteria.source_type = normalize_optional_text("source_type", opt(criteria.source_type), 80)?;
    criteria.media_kind = criteria
        .media_kind
        .map(|value| validate_media_kind(&value))
        .transpose()?
        .flatten();
    criteria.name_query = normalize_optional_text("name_query", opt(criteria.name_query), 160)?;
    criteria.camera_query =
        normalize_optional_text("camera_query", opt(criteria.camera_query), 160)?;
    criteria.keyword_query =
        normalize_optional_text("keyword_query", opt(criteria.keyword_query), 160)?;
    criteria.text_query = normalize_optional_text("text_query", opt(criteria.text_query), 240)?;
    criteria.person_id = normalize_optional_text("person_id", opt(criteria.person_id), 120)?;
    criteria.speaker_id = normalize_optional_text("speaker_id", opt(criteria.speaker_id), 120)?;

    validate_seconds("modified_from", criteria.modified_from)?;
    validate_seconds("modified_to", criteria.modified_to)?;
    validate_seconds("captured_from", criteria.captured_from)?;
    validate_seconds("captured_to", criteria.captured_to)?;
    validate_range("width", criteria.min_width, criteria.max_width)?;
    validate_range("height", criteria.min_height, criteria.max_height)?;
    validate_range(
        "size_bytes",
        criteria.min_size_bytes,
        criteria.max_size_bytes,
    )?;
    validate_float_range("modified", criteria.modified_from, criteria.modified_to)?;
    validate_float_range("captured", criteria.captured_from, criteria.captured_to)?;
    let _ = criteria.duplicate_status;
    let _ = criteria.orientation;
    Ok(criteria)
}

fn opt(value: Option<String>) -> String {
    value.unwrap_or_default()
}

fn validate_media_kind(value: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() || value == "all" {
        return Ok(None);
    }
    match value {
        "static_image" | "animated_gif" | "video_scene" | "audio" | "pdf_page"
        | "pdf_document" => Ok(Some(value.to_string())),
        _ => Err("media_kind must be one of all, static_image, animated_gif, video_scene, audio, pdf_page, pdf_document".to_string()),
    }
}

fn normalize_required_text(name: &str, value: String, max_len: usize) -> Result<String, String> {
    let normalized = normalize_text(name, &value, max_len)?;
    if normalized.is_empty() {
        return Err(format!("{name} is required"));
    }
    Ok(normalized)
}

fn normalize_optional_text(
    name: &str,
    value: String,
    max_len: usize,
) -> Result<Option<String>, String> {
    let normalized = normalize_text(name, &value, max_len)?;
    Ok((!normalized.is_empty()).then_some(normalized))
}

fn normalize_text(name: &str, value: &str, max_len: usize) -> Result<String, String> {
    let value = value.trim();
    if value.chars().any(char::is_control) {
        return Err(format!("{name} cannot contain control characters"));
    }
    if value.chars().count() > max_len {
        return Err(format!("{name} must be {max_len} characters or fewer"));
    }
    Ok(value.to_string())
}

fn validate_seconds(name: &str, value: Option<f64>) -> Result<(), String> {
    if let Some(value) = value {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "{name} must be a non-negative Unix timestamp in seconds"
            ));
        }
    }
    Ok(())
}

fn validate_range<T: PartialOrd>(name: &str, min: Option<T>, max: Option<T>) -> Result<(), String> {
    if min.zip(max).is_some_and(|(min, max)| min > max) {
        return Err(format!(
            "min_{name} must be less than or equal to max_{name}"
        ));
    }
    Ok(())
}

fn validate_float_range(name: &str, min: Option<f64>, max: Option<f64>) -> Result<(), String> {
    if min.zip(max).is_some_and(|(min, max)| min > max) {
        return Err(format!(
            "{name}_from must be less than or equal to {name}_to"
        ));
    }
    Ok(())
}

fn read_album_file(path: &Path) -> Result<SmartAlbumFile, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SmartAlbumFile {
                version: FILE_VERSION,
                albums: Vec::new(),
            });
        }
        Err(error) => return Err(format!("Could not read smart albums file: {error}")),
    };
    let file = serde_json::from_str::<SmartAlbumFile>(&content)
        .map_err(|error| format!("Could not parse smart albums file: {error}"))?;
    if file.version != FILE_VERSION {
        return Err(format!(
            "Unsupported smart albums file version `{}`",
            file.version
        ));
    }
    Ok(file)
}

fn write_album_file(path: &Path, file: &SmartAlbumFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create smart albums directory: {error}"))?;
    }
    let temp_path = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
    let json = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    let mut temp = fs::File::create(&temp_path)
        .map_err(|error| format!("Could not create smart albums temp file: {error}"))?;
    temp.write_all(json.as_bytes())
        .map_err(|error| format!("Could not write smart albums temp file: {error}"))?;
    temp.flush()
        .map_err(|error| format!("Could not flush smart albums temp file: {error}"))?;
    fs::rename(&temp_path, path)
        .map_err(|error| format!("Could not replace smart albums file: {error}"))
}

fn sorted_albums(mut albums: Vec<SmartAlbum>) -> Vec<SmartAlbum> {
    albums.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.name.cmp(&right.name))
    });
    albums
}

fn timestamp() -> String {
    Utc::now().to_rfc3339()
}

#[allow(dead_code)]
fn _assert_serde_enums(_: DuplicateStatusFilter, _: AlbumOrientationFilter) {}

#[cfg(test)]
mod tests {
    use super::{create_smart_album, delete_smart_album, list_smart_albums, update_smart_album};
    use crate::domain::smart_albums::{
        DuplicateStatusFilter, EditableSmartAlbum, SmartAlbumCriteria,
    };

    #[test]
    fn missing_file_loads_empty_album_list() {
        let path = temp_path("missing.json");
        let albums = list_smart_albums(&path).unwrap();
        assert!(albums.albums.is_empty());
    }

    #[test]
    fn invalid_json_returns_clear_error() {
        let path = temp_path("invalid.json");
        std::fs::write(&path, "{not json").unwrap();

        let error = list_smart_albums(&path).unwrap_err();

        assert!(error.contains("Could not parse smart albums file"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn create_update_and_delete_album_roundtrip() {
        let path = temp_path("albums.json");
        let album = create_smart_album(
            &path,
            EditableSmartAlbum {
                name: " PDFs ".to_string(),
                description: Some(" invoices ".to_string()),
                criteria: SmartAlbumCriteria {
                    media_kind: Some("pdf_page".to_string()),
                    duplicate_status: DuplicateStatusFilter::All,
                    ..SmartAlbumCriteria::default()
                },
                sort: Default::default(),
                limit: 60,
            },
        )
        .unwrap();
        assert_eq!(album.name, "PDFs");

        let updated = update_smart_album(
            &path,
            &album.id,
            EditableSmartAlbum {
                name: "Images".to_string(),
                description: None,
                criteria: SmartAlbumCriteria::default(),
                sort: Default::default(),
                limit: 20,
            },
        )
        .unwrap();
        assert_eq!(updated.created_at, album.created_at);
        assert_eq!(updated.limit, 20);

        assert!(delete_smart_album(&path, &album.id).unwrap().deleted);
        assert!(list_smart_albums(&path).unwrap().albums.is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn validation_rejects_bad_values() {
        let path = temp_path("bad.json");
        let error = create_smart_album(
            &path,
            EditableSmartAlbum {
                name: " ".to_string(),
                description: None,
                criteria: SmartAlbumCriteria::default(),
                sort: Default::default(),
                limit: 60,
            },
        )
        .unwrap_err();
        assert!(error.contains("name is required"));
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("smart-albums-{}-{name}", uuid::Uuid::new_v4()))
    }
}

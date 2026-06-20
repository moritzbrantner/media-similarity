use std::fs;
use std::path::Path;

use serde_json;

use super::MediaWorkflowLibrary;

pub fn load_media_workflow_library(path: &Path) -> Result<MediaWorkflowLibrary, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Could not parse {}: {error}", path.display()))
}

pub fn save_media_workflow_library(
    path: &Path,
    library: &MediaWorkflowLibrary,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create workflow config directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let content = serde_json::to_string_pretty(library).map_err(|error| error.to_string())?;
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json")
    ));
    fs::write(&temp, content)
        .map_err(|error| format!("Could not write {}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| {
        format!(
            "Could not move {} to {}: {error}",
            temp.display(),
            path.display()
        )
    })
}

pub fn workflow_file_is_writable(path: &Path) -> bool {
    if path.is_file() {
        return fs::OpenOptions::new().append(true).open(path).is_ok();
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    let probe = parent.join(format!(
        ".processing-workflows-writable-{}",
        std::process::id()
    ));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

use crate::project::UserStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Per-project user metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectMeta {
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub status: Option<String>,
}

/// Data store for all user-defined project metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataStore {
    pub projects: HashMap<String, ProjectMeta>,
}

/// Get the config directory path (~/.config/panoptic/)
fn config_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("panoptic")
}

fn data_file_path() -> PathBuf {
    config_dir().join("data.toml")
}

/// Load user data from disk
pub fn load_data() -> DataStore {
    let path = data_file_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(data) => return data,
                Err(e) => eprintln!("Warning: failed to parse data.toml: {}", e),
            },
            Err(e) => eprintln!("Warning: failed to read data.toml: {}", e),
        }
    }
    DataStore::default()
}

/// Save user data to disk
pub fn save_data(data: &DataStore) -> Result<(), String> {
    let dir = config_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(format!("Failed to create config dir: {}", e));
    }
    match toml::to_string_pretty(data) {
        Ok(content) => std::fs::write(data_file_path(), content)
            .map_err(|e| format!("Failed to write data.toml: {}", e)),
        Err(e) => Err(format!("Failed to serialize data: {}", e)),
    }
}

/// Look up user metadata for a project path, merging it into the project
pub fn apply_user_meta(
    project_path: &Path,
    data: &DataStore,
) -> (Vec<String>, Option<String>, Option<UserStatus>) {
    let key = project_path.to_string_lossy().to_string();
    match data.projects.get(&key) {
        Some(meta) => {
            let status = meta.status.as_deref().and_then(UserStatus::parse);
            (meta.tags.clone(), meta.note.clone(), status)
        }
        None => (Vec::new(), None, None),
    }
}

/// Merge user metadata into projects after scanning
pub fn merge_user_data(projects: &mut [crate::project::Project], data: &DataStore) {
    for project in projects.iter_mut() {
        let (tags, note, status) = apply_user_meta(&project.path, data);
        project.tags = tags;
        project.note = note;
        project.user_status = status;
    }
}

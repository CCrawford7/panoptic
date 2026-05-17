use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A scan root — a directory that panoptic watches for projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRoot {
    pub path: PathBuf,
    pub label: Option<String>,
    pub enabled: bool,
}

impl ScanRoot {
    pub fn new(path: PathBuf) -> Self {
        let label = path.file_name().map(|n| n.to_string_lossy().to_string());
        Self {
            path,
            label,
            enabled: true,
        }
    }

    pub fn label_or_path(&self) -> String {
        self.label
            .clone()
            .or_else(|| {
                self.path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| self.path.to_string_lossy().to_string())
    }
}

/// Wrapper for TOML serialization (TOML requires a table root)
#[derive(Debug, Serialize, Deserialize)]
struct RootsFile {
    root: Vec<ScanRoot>,
}

/// Get the config directory path (~/.config/panoptic/)
fn config_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("panoptic")
}

/// Path to the roots config file
fn roots_file_path() -> PathBuf {
    config_dir().join("roots.toml")
}

/// Load scan roots from disk
pub fn load_roots() -> Vec<ScanRoot> {
    let path = roots_file_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<RootsFile>(&content) {
                Ok(file) => return file.root,
                Err(e) => {
                    eprintln!("Warning: failed to parse roots.toml: {}", e);
                }
            },
            Err(e) => {
                eprintln!("Warning: failed to read roots.toml: {}", e);
            }
        }
    }
    Vec::new()
}

/// Save scan roots to disk
pub fn save_roots(roots: &[ScanRoot]) -> Result<(), String> {
    let dir = config_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(format!("Failed to create config dir: {}", e));
    }

    let file = RootsFile {
        root: roots.to_vec(),
    };

    let path = roots_file_path();
    match toml::to_string_pretty(&file) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                Err(format!("Failed to write roots.toml: {}", e))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(format!("Failed to serialize roots: {}", e)),
    }
}

/// Add a new scan root and persist
pub fn add_root(roots: &mut Vec<ScanRoot>, path: PathBuf) -> Result<(), String> {
    // Resolve the path
    let resolved = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&path))
            .unwrap_or(path)
    };

    // Check for duplicates
    if roots.iter().any(|r| r.path == resolved) {
        return Err(format!("Root already exists: {}", resolved.display()));
    }

    roots.push(ScanRoot::new(resolved));
    save_roots(roots)
}

/// Remove a scan root by index and persist
pub fn remove_root(roots: &mut Vec<ScanRoot>, index: usize) -> Result<(), String> {
    if index >= roots.len() {
        return Err(format!("Invalid root index: {}", index));
    }
    roots.remove(index);
    save_roots(roots)
}

use crate::config::Config;
use crate::git::get_git_state;
use crate::parser::parse_agent_files;
use crate::project::{Project, ProjectType};
use crate::roots::ScanRoot;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Result of scanning a directory
#[derive(Debug, Default)]
pub struct ScanResult {
    pub projects: Vec<Project>,
    pub errors: Vec<String>,
    pub scan_duration_ms: u64,
}

/// Detect the type of a project based on its contents
pub fn detect_project_type(root: &Path) -> ProjectType {
    let has_file = |name: &str| root.join(name).exists();

    // Chrome extension (manifest.json with chrome keys)
    if has_file("manifest.json") {
        if let Ok(content) = std::fs::read_to_string(root.join("manifest.json")) {
            if content.contains("background") || content.contains("content_scripts") {
                return ProjectType::ChromeExtension;
            }
        }
    }

    if has_file("Cargo.toml") {
        return ProjectType::Rust;
    }
    if has_file("package.json") {
        // Check for TypeScript
        if has_file("tsconfig.json") || has_file("tsconfig.tsbuildinfo") {
            return ProjectType::TypeScript;
        }
        // Check if it looks like a web project
        return ProjectType::JavaScript;
    }
    if has_file("pyproject.toml") || has_file("setup.py") || has_file("requirements.txt") {
        return ProjectType::Python;
    }
    if has_file("project.godot") || has_file("project.godot") {
        return ProjectType::Godot;
    }
    if has_file("go.mod") {
        return ProjectType::Go;
    }
    if has_file("flake.nix") || has_file("default.nix") || has_file("shell.nix") {
        return ProjectType::Nix;
    }
    if has_file("Dockerfile") || has_file("docker-compose.yml") {
        return ProjectType::Docker;
    }

    // Check for Godot project via extension scan
    let has_godot_file = std::fs::read_dir(root)
        .ok()
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "godot")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if has_godot_file {
        return ProjectType::Godot;
    }

    // If it has .git but no recognizable type, it's a generic project
    if root.join(".git").exists() {
        return ProjectType::Generic;
    }

    ProjectType::Unknown
}

/// Check if a directory name or path should be ignored
fn should_ignore(entry: &walkdir::DirEntry, config: &Config) -> bool {
    let file_name = entry.file_name().to_string_lossy();

    // Skip hidden dirs/files unless configured
    if !config.show_hidden && file_name.starts_with('.') && file_name != "." {
        return true;
    }

    if config.ignore_dirs.contains(&file_name.to_string()) {
        return true;
    }

    if entry.file_type().is_file() && config.ignore_files.contains(&file_name.to_string()) {
        return true;
    }

    false
}

/// Check if a directory looks like a project root
fn is_project_root(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    // Presence of any of these files suggests a project root
    let project_indicators = [
        ".git",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "setup.py",
        "go.mod",
        "project.godot",
        "flake.nix",
        "default.nix",
        "Dockerfile",
        "manifest.json",
        "Makefile",
        "AGENTS.md",
        "CLAUDE.md",
        "brief.md",
        "PLAN.md",
        "README.md",
    ];

    // Check if it's actually a git repo
    if path.join(".git").exists() {
        return true;
    }

    // Check for other indicators
    for indicator in &project_indicators {
        if path.join(indicator).exists() {
            return true;
        }
    }

    false
}

/// Scan a directory for projects
pub fn scan_directory(path: &Path, config: &Config) -> Result<ScanResult> {
    let start = std::time::Instant::now();
    let mut result = ScanResult::default();

    // Collect candidate directories
    let mut candidates: Vec<PathBuf> = Vec::new();

    let walk_root = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    // Check that the scan root exists
    if !walk_root.exists() {
        result
            .errors
            .push(format!("Scan root does not exist: {}", walk_root.display()));
        result.scan_duration_ms = start.elapsed().as_millis() as u64;
        return Ok(result);
    }

    // Walk the directory tree (no symlink following to avoid loops)
    for entry in WalkDir::new(&walk_root)
        .max_depth(config.max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_ignore(e, config))
    {
        match entry {
            Ok(entry) => {
                // Skip entries we can't read metadata for
                if entry.file_type().is_dir() && entry.depth() > 0 {
                    // Check if this directory might be a project root
                    if is_project_root(entry.path()) {
                        // Make sure it's not the root we're scanning
                        if entry.path() != walk_root {
                            candidates.push(entry.path().to_path_buf());
                            // Don't descend into project subdirectories
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                // Log but don't fail on permission errors, broken symlinks, etc.
                result
                    .errors
                    .push(format!("Skipping {}: {}", e.path().unwrap_or(walk_root.as_path()).display(), e));
            }
        }
    }

    // Also check the root directory itself
    if is_project_root(&walk_root) || !candidates.is_empty() {
        // Root itself might be a project if it has indicators
        let root_has_indicators = walk_root.join(".git").exists()
            || candidates.iter().any(|c| c.parent().is_some_and(|p| p == walk_root));

        if !root_has_indicators {
            // Check if any of the top-level dirs are projects
            if let Ok(entries) = std::fs::read_dir(&walk_root) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        let dir_name = entry.file_name().to_string_lossy().to_string();
                        if !config.show_hidden && dir_name.starts_with('.') {
                            continue;
                        }
                        if config.ignore_dirs.contains(&dir_name) {
                            continue;
                        }

                        let dir_path = entry.path();
                        if is_project_root(&dir_path) {
                            candidates.push(dir_path);
                        }
                    }
                }
            }
        }
    }

    // Deduplicate candidates
    candidates.sort();
    candidates.dedup();

    // Filter out nested projects: if A is a project root and A/B is also one,
    // keep only A (the top-most ancestor). This prevents listing sub-projects
    // that are already contained within a parent project.
    let top_level: Vec<PathBuf> = candidates
        .iter()
        .filter(|c| {
            !candidates.iter().any(|other| {
                other != *c && c.starts_with(other)
            })
        })
        .cloned()
        .collect();
    candidates = top_level;

    // Scan projects in parallel
    let scanned_projects: Vec<Result<Option<Project>>> = candidates
        .par_iter()
        .map(|project_path| scan_single_project(project_path, config))
        .collect();

    for scanned in scanned_projects {
        match scanned {
            Ok(Some(project)) => result.projects.push(project),
            Ok(None) => {}
            Err(e) => result.errors.push(format!("Error scanning: {}", e)),
        }
    }

    // Sort by activity (active first), then by last modified
    result.projects.sort_by(|a, b| {
        b.activity
            .cmp(&a.activity)
            .then_with(|| b.last_modified.cmp(&a.last_modified))
    });

    result.scan_duration_ms = start.elapsed().as_millis() as u64;
    Ok(result)
}

/// Scan multiple roots, merging results (deduplicating by resolved path)
pub fn scan_all_roots(roots: &[ScanRoot], config: &Config) -> Result<ScanResult> {
    let start = std::time::Instant::now();
    let mut result = ScanResult::default();
    let mut seen_paths: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for root in roots.iter().filter(|r| r.enabled) {
        match scan_directory(&root.path, config) {
            Ok(mut sub) => {
                for project in sub.projects.drain(..) {
                    if seen_paths.insert(project.path.clone()) {
                        result.projects.push(project);
                    }
                }
                result.errors.append(&mut sub.errors);
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Error scanning {}: {}", root.path.display(), e));
            }
        }
    }

    // Sort by activity (active first), then by last modified
    result.projects.sort_by(|a, b| {
        b.activity
            .cmp(&a.activity)
            .then_with(|| b.last_modified.cmp(&a.last_modified))
    });

    result.scan_duration_ms = start.elapsed().as_millis() as u64;
    Ok(result)
}

/// Scan a single project directory
fn scan_single_project(project_path: &Path, config: &Config) -> Result<Option<Project>> {
    let project_name = project_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Skip if it's an ignored directory
    if config.ignore_dirs.contains(&project_name) {
        return Ok(None);
    }

    // Skip hidden directories unless configured
    if !config.show_hidden && project_name.starts_with('.') {
        return Ok(None);
    }

    let project_type = detect_project_type(project_path);

    // Calculate size and file count
    let (size, file_count) = calculate_dir_stats(project_path, config);

    // Get last modified time
    let last_modified = get_last_modified(project_path).unwrap_or(Utc::now());

    // Get creation time from .git if available
    let created = get_git_creation_time(project_path);

    // Get git state
    let is_git_repo = project_path.join(".git").exists();
    let git = if is_git_repo {
        get_git_state(project_path).ok()
    } else {
        None
    };

    // Parse agent files
    let agent = parse_agent_files(project_path);

    // Determine activity level
    let days_since = {
        let now = Utc::now();
        let duration = now - last_modified;
        duration.num_days()
    };

    let activity = Project::activity_from_days(days_since);

    let project = Project {
        name: project_name,
        path: project_path.to_path_buf(),
        project_type,
        size,
        file_count,
        last_modified,
        created,
        is_git_repo,
        git,
        agent,
        tags: Vec::new(),
        activity,
    };

    Ok(Some(project))
}

/// Calculate total size and file count for a directory
fn calculate_dir_stats(dir: &Path, config: &Config) -> (u64, u64) {
    let mut total_size: u64 = 0;
    let mut file_count: u64 = 0;

    let mut walk_errors = 0u64;
    for entry in WalkDir::new(dir)
        .max_depth(6)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_ignore(e, config))
    {
        match entry {
            Ok(entry) => {
                if entry.file_type().is_file() {
                    if let Ok(metadata) = entry.metadata() {
                        total_size += metadata.len();
                        file_count += 1;
                    }
                }
            }
            Err(_) => {
                walk_errors += 1;
            }
        }
    }

    // Log if we hit permission errors or broken symlinks
    if walk_errors > 0 {
        eprintln!(
            "Warning: {} entries unreadable in {} (permission denied or broken symlink)",
            walk_errors,
            dir.display()
        );
    }

    (total_size, file_count)
}

/// Get the last modified time of a directory (most recent file)
fn get_last_modified(dir: &Path) -> Option<DateTime<Utc>> {
    let mut latest: Option<DateTime<Utc>> = None;

    for entry in WalkDir::new(dir)
        .max_depth(3)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".git" && name != "node_modules" && name != "target"
        })
    {
        match entry {
            Ok(entry) => {
                if entry.file_type().is_file() || entry.file_type().is_dir() {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(mtime) = metadata.modified() {
                            let datetime: DateTime<Utc> = mtime.into();
                            if latest.is_none_or(|l| datetime > l) {
                                latest = Some(datetime);
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Silently skip unreadable entries
            }
        }
    }

    latest
}

/// Get the creation time of a git repo (first commit date)
fn get_git_creation_time(dir: &Path) -> Option<DateTime<Utc>> {
    let repo = git2::Repository::open(dir).ok()?;
    let mut revwalk = repo.revwalk().ok()?;
    revwalk.push_head().ok()?;
    revwalk.set_sorting(git2::Sort::TIME).ok()?;

    // Get the oldest commit
    let mut oldest: Option<DateTime<Utc>> = None;
    for oid in revwalk.flatten() {
        if let Ok(commit) = repo.find_commit(oid) {
            let time = commit.time();
            let timestamp = time.seconds();
            if let Some(dt) = DateTime::from_timestamp(timestamp, 0) {
                if oldest.is_none_or(|o| dt < o) {
                    oldest = Some(dt);
                }
            }
        }
    }

    oldest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ActivityLevel;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("panoptic-scanner-test-{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_detect_project_type_rust() {
        let dir = temp_dir("rust");
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::Rust);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_typescript() {
        let dir = temp_dir("ts");
        fs::write(dir.join("package.json"), "{}").unwrap();
        fs::write(dir.join("tsconfig.json"), "{}").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::TypeScript);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_javascript() {
        let dir = temp_dir("js");
        fs::write(dir.join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::JavaScript);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_python() {
        let dir = temp_dir("py");
        fs::write(dir.join("pyproject.toml"), "[project]\n").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::Python);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_go() {
        let dir = temp_dir("go");
        fs::write(dir.join("go.mod"), "module test\n").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::Go);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_docker() {
        let dir = temp_dir("docker");
        fs::write(dir.join("Dockerfile"), "FROM ubuntu\n").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::Docker);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_chrome_extension() {
        let dir = temp_dir("chrome");
        fs::write(
            dir.join("manifest.json"),
            r#"{"background": {"scripts": ["bg.js"]}}"#,
        )
        .unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::ChromeExtension);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_generic_git() {
        let dir = temp_dir("generic");
        fs::create_dir(dir.join(".git")).unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::Generic);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_nix() {
        let dir = temp_dir("nix");
        fs::write(dir.join("flake.nix"), "{}").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::Nix);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_detect_project_type_unknown() {
        let dir = temp_dir("unknown");
        fs::write(dir.join("random.txt"), "data").unwrap();
        assert_eq!(detect_project_type(&dir), ProjectType::Unknown);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_is_project_root_with_git() {
        let dir = temp_dir("root-git");
        fs::create_dir(dir.join(".git")).unwrap();
        assert!(is_project_root(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_is_project_root_with_indicator() {
        let dir = temp_dir("root-indicator");
        fs::write(dir.join("Makefile"), "all:\n").unwrap();
        assert!(is_project_root(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_is_project_root_not_root() {
        let dir = temp_dir("not-root");
        assert!(!is_project_root(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_should_ignore_hidden_files() {
        let config = Config::default();
        let dir = temp_dir("ignore-hidden");
        fs::create_dir(dir.join(".hidden")).unwrap();

        let entry = WalkDir::new(&dir).into_iter().filter_entry(|e| !should_ignore(e, &config)).collect::<Vec<_>>();
        let hidden_found = entry.iter().any(|e| {
            e.as_ref().ok().map(|e| e.file_name().to_string_lossy().starts_with('.')).unwrap_or(false)
        });
        // The hidden dir should be filtered out
        assert!(!hidden_found, "hidden dir should be filtered out");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_show_hidden_config() {
        let mut config = Config::default();
        config.show_hidden = true;
        let dir = temp_dir("show-hidden");
        fs::create_dir(dir.join(".visible")).unwrap();

        let entries: Vec<_> = WalkDir::new(&dir)
            .into_iter()
            .filter_entry(|e| !should_ignore(e, &config))
            .collect();
        let hidden_found = entries.iter().any(|e| {
            e.as_ref().ok().map(|e| e.file_name().to_string_lossy() == ".visible").unwrap_or(false)
        });
        assert!(hidden_found, "hidden dir should be visible when show_hidden is true");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_scan_directory_no_projects() {
        let dir = temp_dir("empty-scan");
        let config = Config::default();
        let result = scan_directory(&dir, &config).unwrap();
        assert_eq!(result.projects.len(), 0);
        assert!(result.errors.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_scan_directory_finds_projects() {
        let dir = temp_dir("multi-scan");

        // Create two project directories
        fs::create_dir(dir.join("proj-a")).unwrap();
        fs::write(dir.join("proj-a/Cargo.toml"), "[package]\nname = \"a\"\n").unwrap();

        fs::create_dir(dir.join("proj-b")).unwrap();
        fs::write(dir.join("proj-b/package.json"), "{}").unwrap();

        let config = Config::default();
        let result = scan_directory(&dir, &config).unwrap();

        assert_eq!(result.projects.len(), 2);
        let names: Vec<&str> = result.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"proj-a"));
        assert!(names.contains(&"proj-b"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_scan_directory_skips_ignored() {
        let dir = temp_dir("ignored-scan");

        fs::create_dir(dir.join("node_modules")).unwrap();
        fs::write(dir.join("node_modules/package.json"), "{}").unwrap();

        let config = Config::default();
        let result = scan_directory(&dir, &config).unwrap();
        assert_eq!(result.projects.len(), 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_activity_level_from_modification() {
        // Verify activity classification boundaries
        assert_eq!(Project::activity_from_days(0), ActivityLevel::Active);
        assert_eq!(Project::activity_from_days(29), ActivityLevel::Active);
        assert_eq!(Project::activity_from_days(30), ActivityLevel::Active);
        assert_eq!(Project::activity_from_days(31), ActivityLevel::Stable);
        assert_eq!(Project::activity_from_days(89), ActivityLevel::Stable);
        assert_eq!(Project::activity_from_days(90), ActivityLevel::Stable);
        assert_eq!(Project::activity_from_days(91), ActivityLevel::Stale);
    }
}

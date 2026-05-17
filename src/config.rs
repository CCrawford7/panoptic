use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for panoptic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Directories to scan (default: current dir)
    pub scan_dirs: Vec<PathBuf>,
    /// Maximum depth to scan
    pub max_depth: usize,
    /// Patterns to ignore (directory names)
    pub ignore_dirs: Vec<String>,
    /// Patterns to ignore (file names)
    pub ignore_files: Vec<String>,
    /// Minimum git commit age in days for stale detection
    pub stale_days: u64,
    /// Minimum git commit age in days for active detection
    pub active_days: u64,
    /// Web server port
    pub web_port: u16,
    /// Whether to open browser automatically
    pub web_open_browser: bool,
    /// Max projects to show in overview
    pub max_projects: usize,
    /// Show hidden directories (starting with .)
    pub show_hidden: bool,
    /// Editor command for quick actions (e.g., "code", "vim")
    pub editor: Option<String>,
    /// Terminal command for quick actions (e.g., "alacritty", "gnome-terminal")
    pub terminal: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan_dirs: vec![PathBuf::from(".")],
            max_depth: 3,
            ignore_dirs: vec![
                "node_modules".into(),
                "target".into(),
                ".git".into(),
                ".next".into(),
                ".cache".into(),
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
                "dist".into(),
                "build".into(),
                ".svelte-kit".into(),
                ".bmad-core".into(),
                ".bmad-godot-game-dev".into(),
            ],
            ignore_files: vec![".DS_Store".into(), "Thumbs.db".into(), ".gitkeep".into()],
            stale_days: 90,
            active_days: 30,
            web_port: 3173,
            web_open_browser: true,
            max_projects: 200,
            show_hidden: false,
            editor: None,
            terminal: None,
        }
    }
}

impl Config {
    /// Get the editor command, with auto-detection fallback
    pub fn editor_cmd(&self) -> String {
        self.editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    "open".to_string()
                } else if cfg!(target_os = "windows") {
                    "code".to_string()
                } else {
                    "xdg-open".to_string()
                }
            })
    }

    /// Get the terminal command, with auto-detection fallback
    pub fn terminal_cmd(&self) -> String {
        self.terminal
            .clone()
            .or_else(|| std::env::var("TERMINAL").ok())
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    "open".to_string()
                } else if cfg!(target_os = "windows") {
                    "cmd".to_string()
                } else {
                    "x-terminal-emulator".to_string()
                }
            })
    }

    /// Get the flag to set working directory for a terminal
    pub fn terminal_cwd_flag(term: &str) -> &'static str {
        match term {
            "alacritty" | "Alacritty" => "--working-directory",
            "gnome-terminal" | "gnome-terminal." => "--working-directory",
            "kitty" => "--directory",
            "wezterm" => "--cwd",
            "konsole" => "--workdir",
            _ => "--working-directory", // best guess
        }
    }
    /// Load config from a TOML file, merging with defaults
    pub fn load(path: Option<&PathBuf>) -> Result<Self> {
        let mut config = Config::default();

        if let Some(path) = path {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let file_config: Config = toml::from_str(&content)?;
                config.merge(file_config);
            }
        }

        Ok(config)
    }

    fn merge(&mut self, other: Config) {
        if !other.scan_dirs.is_empty() {
            self.scan_dirs = other.scan_dirs;
        }
        self.max_depth = other.max_depth;
        if !other.ignore_dirs.is_empty() {
            self.ignore_dirs = other.ignore_dirs;
        }
        if !other.ignore_files.is_empty() {
            self.ignore_files = other.ignore_files;
        }
        self.stale_days = other.stale_days;
        self.active_days = other.active_days;
        self.web_port = other.web_port;
        self.web_open_browser = other.web_open_browser;
        self.max_projects = other.max_projects;
        self.show_hidden = other.show_hidden;
        if other.editor.is_some() {
            self.editor = other.editor;
        }
        if other.terminal.is_some() {
            self.terminal = other.terminal;
        }
    }

    /// Generate a default config file content
    pub fn generate_example() -> String {
        r#"# Panoptic Configuration
# Place this file at ~/.config/panoptic/config.toml or in the scanned directory

# Directories to scan (default: current directory)
# scan_dirs = ["."]

# Maximum directory depth to scan for projects
# max_depth = 3

# Directory names to ignore
# ignore_dirs = ["node_modules", "target", ".git"]

# Web server port
# web_port = 3173

# Open browser automatically when starting web mode
# web_open_browser = true
"#
        .to_string()
    }
}

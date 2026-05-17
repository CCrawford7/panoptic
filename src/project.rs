use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The type of a detected project
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Godot,
    Go,
    Nix,
    Docker,
    ChromeExtension,
    Generic,
    Unknown,
}

impl ProjectType {
    pub fn label(&self) -> &'static str {
        match self {
            ProjectType::Rust => "Rust",
            ProjectType::TypeScript => "TypeScript",
            ProjectType::JavaScript => "JavaScript",
            ProjectType::Python => "Python",
            ProjectType::Godot => "Godot",
            ProjectType::Go => "Go",
            ProjectType::Nix => "Nix",
            ProjectType::Docker => "Docker",
            ProjectType::ChromeExtension => "Chrome Ext",
            ProjectType::Generic => "Generic",
            ProjectType::Unknown => "Unknown",
        }
    }
}

/// Activity level of a project
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActivityLevel {
    Active,  // modified within last 30 days
    Stable,  // modified within last 90 days
    Stale,   // modified > 90 days ago
    Done,    // tagged as complete
    Archived,
}

impl ActivityLevel {
    pub fn label(&self) -> &'static str {
        match self {
            ActivityLevel::Active => "Active",
            ActivityLevel::Stable => "Stable",
            ActivityLevel::Stale => "Stale",
            ActivityLevel::Done => "Done",
            ActivityLevel::Archived => "Archived",
        }
    }
}

/// Git state of a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitState {
    pub branch: String,
    pub is_dirty: bool,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub ahead: u32,
    pub behind: u32,
    pub last_commit_time: Option<DateTime<Utc>>,
    pub last_commit_message: Option<String>,
    pub last_commit_author: Option<String>,
    pub total_commits: u32,
    pub stash_count: u32,
    pub has_remote: bool,
}

impl GitState {
    pub fn health_label(&self) -> &'static str {
        if self.is_dirty {
            "dirty"
        } else if self.ahead > 0 || self.behind > 0 {
            "diverged"
        } else {
            "clean"
        }
    }
}

/// Parsed context from agent files, READMEs, and docs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentContext {
    /// One-line description (from README or brief.md first paragraph)
    pub description: Option<String>,
    /// Current development phase (from CLAUDE.md/AGENTS.md)
    pub current_phase: Option<String>,
    /// Current active task
    pub current_task: Option<String>,
    /// Next steps / todo items
    pub next_steps: Vec<String>,
    /// Blocker descriptions
    pub blockers: Vec<String>,
    /// Completed checklist items
    pub completed_items: Vec<String>,
    /// Recent decisions made
    pub recent_decisions: Vec<String>,
    /// Total checklist items found
    pub checklist_total: u32,
    /// Completed checklist items count
    pub checklist_done: u32,
}

/// Tag assigned to a project
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectTag {
    Game,
    Tool,
    Web,
    Mobile,
    Backend,
    Experiment,
    Prototype,
    Custom(String),
}

/// A single detected project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub project_type: ProjectType,
    pub size: u64,          // bytes
    pub file_count: u64,
    pub last_modified: DateTime<Utc>,
    pub created: Option<DateTime<Utc>>,
    pub is_git_repo: bool,
    pub git: Option<GitState>,
    pub agent: Option<AgentContext>,
    pub tags: Vec<String>,
    pub activity: ActivityLevel,
}

impl Project {
    pub fn size_human(&self) -> String {
        let bytes = self.size as f64;
        if bytes < 1024.0 {
            format!("{}B", bytes as u64)
        } else if bytes < 1024.0 * 1024.0 {
            format!("{:.0}K", bytes / 1024.0)
        } else if bytes < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1}M", bytes / (1024.0 * 1024.0))
        } else {
            format!("{:.1}G", bytes / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn file_count_human(&self) -> String {
        if self.file_count >= 1000 {
            format!("{:.1}k", self.file_count as f64 / 1000.0)
        } else {
            format!("{}", self.file_count)
        }
    }

    pub fn days_since_modified(&self) -> i64 {
        let now = Utc::now();
        let duration = now - self.last_modified;
        duration.num_days()
    }

    pub fn activity_from_days(days: i64) -> ActivityLevel {
        if days <= 30 {
            ActivityLevel::Active
        } else if days <= 90 {
            ActivityLevel::Stable
        } else {
            ActivityLevel::Stale
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_type_labels() {
        assert_eq!(ProjectType::Rust.label(), "Rust");
        assert_eq!(ProjectType::TypeScript.label(), "TypeScript");
        assert_eq!(ProjectType::Python.label(), "Python");
        assert_eq!(ProjectType::Godot.label(), "Godot");
        assert_eq!(ProjectType::ChromeExtension.label(), "Chrome Ext");
        assert_eq!(ProjectType::Unknown.label(), "Unknown");
    }

    #[test]
    fn test_activity_level_labels() {
        assert_eq!(ActivityLevel::Active.label(), "Active");
        assert_eq!(ActivityLevel::Stable.label(), "Stable");
        assert_eq!(ActivityLevel::Stale.label(), "Stale");
        assert_eq!(ActivityLevel::Done.label(), "Done");
        assert_eq!(ActivityLevel::Archived.label(), "Archived");
    }

    #[test]
    fn test_activity_from_days() {
        assert_eq!(Project::activity_from_days(0), ActivityLevel::Active);
        assert_eq!(Project::activity_from_days(15), ActivityLevel::Active);
        assert_eq!(Project::activity_from_days(30), ActivityLevel::Active);
        assert_eq!(Project::activity_from_days(31), ActivityLevel::Stable);
        assert_eq!(Project::activity_from_days(60), ActivityLevel::Stable);
        assert_eq!(Project::activity_from_days(90), ActivityLevel::Stable);
        assert_eq!(Project::activity_from_days(91), ActivityLevel::Stale);
        assert_eq!(Project::activity_from_days(365), ActivityLevel::Stale);
    }

    #[test]
    fn test_activity_ordering() {
        // Active should come first (sort uses reverse order)
        assert!(ActivityLevel::Active < ActivityLevel::Stable);
        assert!(ActivityLevel::Stable < ActivityLevel::Stale);
        assert!(ActivityLevel::Stale < ActivityLevel::Done);
        assert!(ActivityLevel::Done < ActivityLevel::Archived);
    }

    #[test]
    fn test_size_human() {
        let make_project = |size: u64| Project {
            name: "test".into(),
            path: PathBuf::from("/tmp/test"),
            project_type: ProjectType::Generic,
            size,
            file_count: 0,
            last_modified: Utc::now(),
            created: None,
            is_git_repo: false,
            git: None,
            agent: None,
            tags: vec![],
            activity: ActivityLevel::Active,
        };

        assert_eq!(make_project(500).size_human(), "500B");
        assert_eq!(make_project(1024).size_human(), "1K");
        assert_eq!(make_project(1536).size_human(), "2K");
        assert_eq!(make_project(1_048_576).size_human(), "1.0M");
        assert_eq!(make_project(1_073_741_824).size_human(), "1.0G");
    }

    #[test]
    fn test_file_count_human() {
        let make_project = |file_count: u64| Project {
            name: "test".into(),
            path: PathBuf::from("/tmp/test"),
            project_type: ProjectType::Generic,
            size: 0,
            file_count,
            last_modified: Utc::now(),
            created: None,
            is_git_repo: false,
            git: None,
            agent: None,
            tags: vec![],
            activity: ActivityLevel::Active,
        };

        assert_eq!(make_project(42).file_count_human(), "42");
        assert_eq!(make_project(999).file_count_human(), "999");
        assert_eq!(make_project(1000).file_count_human(), "1.0k");
        assert_eq!(make_project(1500).file_count_human(), "1.5k");
        assert_eq!(make_project(12345).file_count_human(), "12.3k");
    }

    #[test]
    fn test_git_health_labels() {
        let make_state = |is_dirty: bool, ahead: u32, behind: u32| GitState {
            branch: "main".into(),
            is_dirty,
            staged: 0,
            unstaged: 0,
            untracked: 0,
            ahead,
            behind,
            last_commit_time: None,
            last_commit_message: None,
            last_commit_author: None,
            total_commits: 5,
            stash_count: 0,
            has_remote: true,
        };

        assert_eq!(make_state(false, 0, 0).health_label(), "clean");
        assert_eq!(make_state(true, 0, 0).health_label(), "dirty");
        assert_eq!(make_state(false, 3, 0).health_label(), "diverged");
        assert_eq!(make_state(false, 0, 2).health_label(), "diverged");
        assert_eq!(make_state(true, 3, 1).health_label(), "dirty"); // dirty takes priority
    }
}

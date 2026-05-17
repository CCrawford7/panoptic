use anyhow::Result;
use clap::{Parser, Subcommand};
use panoptic::config::Config;
use std::path::PathBuf;

/// Panoptic - The all-seeing project dashboard for your dev folders.
///
/// Scans directories for projects and presents a beautiful birds-eye view
/// with git status, agent context, and activity tracking.
///
/// You can specify one or more directories to scan. Multiple roots are
/// persisted and merged into a single unified dashboard.
#[derive(Parser, Debug)]
#[command(name = "panoptic", version, about = "The all-seeing project dashboard")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Directories to scan for projects (default: current directory).
    /// Multiple paths can be specified: panoptic ~/code ~/games
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Start in web dashboard mode instead of TUI
    #[arg(long, short = 'w')]
    web: bool,

    /// Port for web dashboard (default: 3173)
    #[arg(long, short = 'p', default_value_t = 3173)]
    port: u16,

    /// Config file path
    #[arg(long, short = 'c')]
    config: Option<PathBuf>,

    /// Don't open browser in web mode
    #[arg(long)]
    no_browser: bool,

    /// Show hidden directories
    #[arg(long)]
    show_hidden: bool,

    /// Maximum scan depth
    #[arg(long, default_value_t = 3)]
    max_depth: usize,

    /// Output JSON and exit
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Export project context as a Markdown summary
    Export {
        /// Name or path fragment of the project to export
        name: String,
        /// Scan roots to search (default: current directory)
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration
    let mut config = panoptic::config::Config::load(cli.config.as_ref())?;

    // Override with CLI flags
    if cli.port != 3173 {
        config.web_port = cli.port;
    }
    if cli.no_browser {
        config.web_open_browser = false;
    }
    if cli.show_hidden {
        config.show_hidden = true;
    }
    if cli.max_depth != 3 {
        config.max_depth = cli.max_depth;
    }

    // Handle subcommands
    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Export { name, paths } => {
                return cmd_export(&name, &paths, &config);
            }
        }
    }

    // Resolve all scan paths
    let scan_paths: Vec<PathBuf> = cli
        .paths
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(p))
                    .unwrap_or_else(|_| p.clone())
            }
        })
        .collect();

    // If --json, scan and output
    if cli.json {
        let roots: Vec<panoptic::roots::ScanRoot> = scan_paths
            .iter()
            .map(|p| panoptic::roots::ScanRoot::new(p.clone()))
            .collect();
        let result = panoptic::scanner::scan_all_roots(&roots, &config)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "scan_duration_ms": result.scan_duration_ms,
                "project_count": result.projects.len(),
                "projects": result.projects.iter().map(|p| {
                    serde_json::json!({
                        "name": p.name,
                        "path": p.path.to_string_lossy(),
                        "type": p.project_type.label(),
                        "size": p.size,
                        "size_human": p.size_human(),
                        "file_count": p.file_count,
                        "activity": p.activity.label(),
                        "last_modified": p.last_modified.to_rfc3339(),
                        "days_since_modified": p.days_since_modified(),
                        "is_git_repo": p.is_git_repo,
                        "git": p.git,
                        "agent": p.agent,
                    })
                }).collect::<Vec<_>>()
            }))?
        );
        return Ok(());
    }

    // Build ScanRoots from resolved paths
    let roots: Vec<panoptic::roots::ScanRoot> = scan_paths
        .iter()
        .map(|p| panoptic::roots::ScanRoot::new(p.clone()))
        .collect();

    if cli.web {
        // Web mode — supports multiple roots
        panoptic::web::start(scan_paths, config).await?;
    } else {
        // TUI mode — supports multiple roots
        let mut app = panoptic::tui::TuiApp::new(config);
        app.run(roots)?;
    }

    Ok(())
}

/// Export a project's context as a markdown summary
fn cmd_export(name: &str, paths: &[PathBuf], config: &Config) -> Result<()> {
    // Build roots from paths
    let roots: Vec<panoptic::roots::ScanRoot> = paths
        .iter()
        .map(|p| {
            let resolved = if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(p))
                    .unwrap_or_else(|_| p.clone())
            };
            panoptic::roots::ScanRoot::new(resolved)
        })
        .collect();

    let result = panoptic::scanner::scan_all_roots(&roots, config)?;

    // Find matching project
    let name_lower = name.to_lowercase();
    let project = result.projects.iter().find(|p| {
        p.name.to_lowercase() == name_lower
            || p.name.to_lowercase().contains(&name_lower)
            || p.path
                .to_string_lossy()
                .to_lowercase()
                .contains(&name_lower)
    });

    match project {
        Some(p) => {
            println!("# {}", p.name);
            println!();
            println!("**Path:** `{}`", p.path.display());
            println!(
                "**Type:** {} | **Activity:** {}",
                p.project_type.label(),
                p.activity.label()
            );
            println!(
                "**Size:** {} ({}) | **Files:** {}",
                p.size_human(),
                p.size,
                p.file_count
            );
            println!("**Modified:** {} days ago", p.days_since_modified());

            // User metadata
            if let Some(status) = &p.user_status {
                println!("**Status:** {}", status.label());
            }
            if !p.tags.is_empty() {
                println!("**Tags:** {}", p.tags.join(", "));
            }
            if let Some(note) = &p.note {
                println!("**Note:** {}", note);
            }
            println!();

            // Git
            if let Some(git) = &p.git {
                println!("## Git");
                println!();
                println!("- Branch: `{}`", git.branch);
                println!("- Status: {}", if git.is_dirty { "dirty" } else { "clean" });
                if git.staged > 0 || git.unstaged > 0 || git.untracked > 0 {
                    println!(
                        "- Changes: +{} staged, +{} unstaged, {} untracked",
                        git.staged, git.unstaged, git.untracked
                    );
                }
                if git.ahead > 0 || git.behind > 0 {
                    println!("- Remote: {} ahead, {} behind", git.ahead, git.behind);
                }
                if let Some(msg) = &git.last_commit_message {
                    println!("- Last commit: {}", msg);
                }
                println!("- Total commits: {}", git.total_commits);
                if git.has_remote {
                    println!("- Remote: configured");
                }
                println!();
            }

            // Agent context
            if let Some(agent) = &p.agent {
                println!("## Agent Context");
                println!();
                if let Some(desc) = &agent.description {
                    println!("{}", desc);
                    println!();
                }
                if let Some(phase) = &agent.current_phase {
                    println!("**Phase:** {}", phase);
                }
                if let Some(task) = &agent.current_task {
                    println!("**Current Task:** {}", task);
                }
                if agent.checklist_total > 0 {
                    println!(
                        "**Progress:** {}/{} ({:.0}%)",
                        agent.checklist_done,
                        agent.checklist_total,
                        agent.checklist_done as f64 / agent.checklist_total as f64 * 100.0
                    );
                }
                if !agent.next_steps.is_empty() {
                    println!();
                    println!("### Next Steps");
                    for step in &agent.next_steps {
                        println!("- [ ] {}", step);
                    }
                }
                if !agent.blockers.is_empty() {
                    println!();
                    println!("### Blockers");
                    for blocker in &agent.blockers {
                        println!("- ❌ {}", blocker);
                    }
                }
                if !agent.recent_decisions.is_empty() {
                    println!();
                    println!("### Recent Decisions");
                    for decision in &agent.recent_decisions {
                        println!("- → {}", decision);
                    }
                }
                println!();
            }

            // Dependencies
            if !p.dependencies.is_empty() {
                println!("## Dependencies ({})", p.dependencies.len());
                println!();
                for dep in &p.dependencies {
                    let cat = match dep.category {
                        panoptic::project::DepCategory::Runtime => "",
                        panoptic::project::DepCategory::Dev => " (dev)",
                        panoptic::project::DepCategory::Build => " (build)",
                        panoptic::project::DepCategory::Optional => " (optional)",
                    };
                    println!("- `{}` **{}**{}", dep.name, dep.version, cat);
                }
                println!();
            }

            Ok(())
        }
        None => {
            eprintln!("Error: No project found matching '{}'", name);
            eprintln!(
                "Scanned {} projects in {} roots:",
                result.projects.len(),
                roots.len()
            );
            for p in &result.projects {
                eprintln!("  - {} ({})", p.name, p.path.display());
            }
            std::process::exit(1);
        }
    }
}

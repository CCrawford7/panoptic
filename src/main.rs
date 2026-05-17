use anyhow::Result;
use clap::Parser;
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
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
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
        }))?);
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

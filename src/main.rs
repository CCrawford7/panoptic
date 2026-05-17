use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Panoptic - The all-seeing project dashboard for your dev folders.
///
/// Scans a directory for projects and presents a beautiful birds-eye view
/// with git status, agent context, and activity tracking.
#[derive(Parser, Debug)]
#[command(name = "panoptic", version, about = "The all-seeing project dashboard")]
struct Cli {
    /// Directory to scan for projects (default: current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

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

    let scan_path = if cli.path.is_absolute() {
        cli.path.clone()
    } else {
        std::env::current_dir()?.join(&cli.path)
    };

    // If --json, just scan and output
    if cli.json {
        let result = panoptic::scanner::scan_directory(&scan_path, &config)?;
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

    if cli.web {
        // Web mode
        panoptic::web::start(scan_path, config).await?;
    } else {
        // TUI mode
        let mut app = panoptic::tui::TuiApp::new(config);
        app.run(scan_path)?;
    }

    Ok(())
}

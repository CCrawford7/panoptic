pub mod routes;

use crate::config::Config;
use crate::project::Project;
use crate::roots::{load_roots, save_roots, ScanRoot};
use crate::scanner::scan_all_roots;
use anyhow::Result;
use axum::Router;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

/// Shared application state for the web server
pub struct AppState {
    pub projects: Vec<Project>,
    pub config: Config,
    pub roots: Vec<ScanRoot>,
}

pub type SharedState = Arc<Mutex<AppState>>;

/// Start the web server
pub async fn start(scan_paths: Vec<PathBuf>, config: Config) -> Result<()> {
    // Load persisted roots, merge with any paths from CLI
    let mut roots = load_roots();

    for path in &scan_paths {
        let resolved = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()?.join(path)
        };
        if !roots.iter().any(|r| r.path == resolved) {
            roots.push(ScanRoot::new(resolved));
        }
    }

    // If no roots at all, add current dir
    if roots.is_empty() {
        roots.push(ScanRoot::new(std::env::current_dir()?));
    }

    // Persist any new roots
    let _ = save_roots(&roots);

    // Scan all roots
    println!("🔍 Scanning {} root(s)...", roots.len());
    for r in &roots {
        println!("   📁 {} ({})", r.label_or_path(), r.path.display());
    }
    let scan_result = scan_all_roots(&roots, &config)?;

    println!(
        "✅ Found {} projects in {}ms",
        scan_result.projects.len(),
        scan_result.scan_duration_ms
    );

    // Print results per root
    for r in &roots {
        let count = scan_result
            .projects
            .iter()
            .filter(|p| p.path.starts_with(&r.path))
            .count();
        println!("   {}: {} projects", r.label_or_path(), count);
    }

    let state = Arc::new(Mutex::new(AppState {
        projects: scan_result.projects,
        config: config.clone(),
        roots,
    }));

    let app = Router::new()
        .route("/", axum::routing::get(routes::index))
        .route("/api/projects", axum::routing::get(routes::get_projects))
        .route("/api/projects/:index", axum::routing::get(routes::get_project))
        .route("/api/stats", axum::routing::get(routes::get_stats))
        .route("/api/refresh", axum::routing::post(routes::refresh))
        .route("/api/roots", axum::routing::get(routes::get_roots))
        .route("/api/roots", axum::routing::post(routes::add_root))
        .route("/api/roots/:index", axum::routing::delete(routes::remove_root))
        .route("/api/roots/:index", axum::routing::patch(routes::update_root))
        .route("/static/*path", axum::routing::get(routes::static_files))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.web_port);
    println!("🌐 Panoptic web dashboard: http://localhost:{}", config.web_port);
    println!("   API: http://localhost:{}/api/projects", config.web_port);
    println!("   Press Ctrl+C to stop");

    // Try to open browser
    if config.web_open_browser {
        let url = format!("http://localhost:{}", config.web_port);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = open::that(&url);
        });
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

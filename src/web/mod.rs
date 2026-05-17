pub mod routes;

use crate::config::Config;
use crate::project::Project;
use crate::scanner::scan_directory;
use anyhow::Result;
use axum::Router;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

/// Shared application state for the web server
pub struct AppState {
    pub projects: Vec<Project>,
    pub config: Config,
    pub scan_path: PathBuf,
}

pub type SharedState = Arc<Mutex<AppState>>;

/// Start the web server
pub async fn start(scan_path: PathBuf, config: Config) -> Result<()> {
    // Scan projects
    println!("🔍 Scanning projects in {:?}...", scan_path);
    let scan_result = scan_directory(&scan_path, &config)?;
    println!(
        "✅ Found {} projects in {}ms",
        scan_result.projects.len(),
        scan_result.scan_duration_ms
    );

    let state = Arc::new(Mutex::new(AppState {
        projects: scan_result.projects,
        config: config.clone(),
        scan_path,
    }));

    let app = Router::new()
        .route("/", axum::routing::get(routes::index))
        .route("/api/projects", axum::routing::get(routes::get_projects))
        .route("/api/projects/:index", axum::routing::get(routes::get_project))
        .route("/api/stats", axum::routing::get(routes::get_stats))
        .route("/api/refresh", axum::routing::post(routes::refresh))
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

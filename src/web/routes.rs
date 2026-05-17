use crate::web::SharedState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct StaticAssets;

/// Serve the main HTML page
pub async fn index() -> impl IntoResponse {
    match StaticAssets::get("index.html") {
        Some(content) => Html(content.data.into_owned()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            "index.html not found (build with static/ directory)",
        )
            .into_response(),
    }
}

/// Serve embedded static files
pub async fn static_files(Path(path): Path<String>) -> (StatusCode, [(axum::http::header::HeaderName, &'static str); 1], Vec<u8>) {
    let path = if path.is_empty() { "index.html" } else { &path };
    match StaticAssets::get(path) {
        Some(content) => {
            let mime = mime_type(path);
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime)],
                content.data.into_owned(),
            )
        }
        None => (
            StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            b"Not found".to_vec(),
        ),
    }
}

fn mime_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "text/plain; charset=utf-8"
    }
}

/// Get all projects as JSON
pub async fn get_projects(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let state = state.lock().unwrap();
    let projects = &state.projects;

    let json: Vec<serde_json::Value> = projects
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "path": p.path.to_string_lossy(),
                "type": p.project_type.label(),
                "size": p.size,
                "size_human": p.size_human(),
                "file_count": p.file_count,
                "activity": p.activity.label(),
                "days_since_modified": p.days_since_modified(),
                "is_git_repo": p.is_git_repo,
                "git": p.git.as_ref().map(|g| {
                    serde_json::json!({
                        "branch": g.branch,
                        "is_dirty": g.is_dirty,
                        "staged": g.staged,
                        "unstaged": g.unstaged,
                        "untracked": g.untracked,
                        "ahead": g.ahead,
                        "behind": g.behind,
                        "health": g.health_label(),
                        "last_commit_message": g.last_commit_message,
                        "total_commits": g.total_commits,
                    })
                }),
                "agent": p.agent.as_ref().map(|a| {
                    serde_json::json!({
                        "description": a.description,
                        "current_phase": a.current_phase,
                        "current_task": a.current_task,
                        "next_steps": a.next_steps,
                        "blockers": a.blockers,
                        "completed_items": a.completed_items,
                        "recent_decisions": a.recent_decisions,
                        "checklist_total": a.checklist_total,
                        "checklist_done": a.checklist_done,
                    })
                }),
            })
        })
        .collect();

    Json(serde_json::json!({
        "count": projects.len(),
        "projects": json
    }))
}

/// Get a single project by index
pub async fn get_project(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
) -> Json<serde_json::Value> {
    let state = state.lock().unwrap();
    if let Some(project) = state.projects.get(index) {
        Json(serde_json::json!({
            "found": true,
            "project": {
                "name": project.name,
                "path": project.path.to_string_lossy(),
                "type": project.project_type.label(),
                "size": project.size,
                "size_human": project.size_human(),
                "file_count": project.file_count,
                "file_count_human": project.file_count_human(),
                "activity": project.activity.label(),
                "days_since_modified": project.days_since_modified(),
                "is_git_repo": project.is_git_repo,
                "last_modified": project.last_modified.to_rfc3339(),
                "git": project.git.as_ref().map(|g| {
                    serde_json::json!({
                        "branch": g.branch,
                        "is_dirty": g.is_dirty,
                        "staged": g.staged,
                        "unstaged": g.unstaged,
                        "untracked": g.untracked,
                        "ahead": g.ahead,
                        "behind": g.behind,
                        "health": g.health_label(),
                        "last_commit_time": g.last_commit_time,
                        "last_commit_message": g.last_commit_message,
                        "last_commit_author": g.last_commit_author,
                        "total_commits": g.total_commits,
                        "stash_count": g.stash_count,
                        "has_remote": g.has_remote,
                    })
                }),
                "agent": project.agent.as_ref().map(|a| {
                    serde_json::json!({
                        "description": a.description,
                        "current_phase": a.current_phase,
                        "current_task": a.current_task,
                        "next_steps": a.next_steps,
                        "blockers": a.blockers,
                        "checklist_total": a.checklist_total,
                        "checklist_done": a.checklist_done,
                        "completed_items": a.completed_items,
                        "recent_decisions": a.recent_decisions,
                    })
                }),
            }
        }))
    } else {
        Json(serde_json::json!({"found": false}))
    }
}

/// Get aggregate stats
pub async fn get_stats(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let state = state.lock().unwrap();
    let projects = &state.projects;

    let total = projects.len();
    let active = projects.iter().filter(|p| p.activity.label() == "Active").count();
    let dirty = projects.iter().filter(|p| p.git.as_ref().map(|g| g.is_dirty).unwrap_or(false)).count();
    let git_repos = projects.iter().filter(|p| p.is_git_repo).count();
    let with_context = projects.iter().filter(|p| p.agent.is_some()).count();

    // Count by type
    let mut type_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for p in projects {
        *type_counts.entry(p.project_type.label().to_string()).or_insert(0) += 1;
    }

    // Total size
    let total_size: u64 = projects.iter().map(|p| p.size).sum();
    let total_size_human = {
        let bytes = total_size as f64;
        if bytes < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", bytes / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes / (1024.0 * 1024.0 * 1024.0))
        }
    };

    Json(serde_json::json!({
        "total": total,
        "active": active,
        "dirty": dirty,
        "git_repos": git_repos,
        "with_agent_context": with_context,
        "total_size": total_size,
        "total_size_human": total_size_human,
        "type_breakdown": type_counts,
    }))
}

/// Rescan projects
pub async fn refresh(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut state = state.lock().unwrap();
    let scan_path = state.scan_path.clone();
    let config = &state.config;

    // Rescan
    if let Ok(result) = crate::scanner::scan_directory(&scan_path, config) {
        let count = result.projects.len();
        state.projects = result.projects;
        state.projects.sort_by(|a, b| {
            b.activity
                .cmp(&a.activity)
                .then_with(|| b.last_modified.cmp(&a.last_modified))
        });

        Json(serde_json::json!({
            "status": "ok",
            "projects_found": count
        }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "projects_found": 0
        }))
    }
}

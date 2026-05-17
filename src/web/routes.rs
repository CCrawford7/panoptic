use crate::project::UserStatus;
use crate::web::SharedState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use rust_embed::RustEmbed;
use std::process::Command as StdCommand;

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
pub async fn static_files(
    Path(path): Path<String>,
) -> (
    StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    Vec<u8>,
) {
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
    let active = projects
        .iter()
        .filter(|p| p.activity.label() == "Active")
        .count();
    let dirty = projects
        .iter()
        .filter(|p| p.git.as_ref().map(|g| g.is_dirty).unwrap_or(false))
        .count();
    let git_repos = projects.iter().filter(|p| p.is_git_repo).count();
    let with_context = projects.iter().filter(|p| p.agent.is_some()).count();

    // Count by type
    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for p in projects {
        *type_counts
            .entry(p.project_type.label().to_string())
            .or_insert(0) += 1;
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

/// Rescan all roots
pub async fn refresh(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut state = state.lock().unwrap();
    let roots = state.roots.clone();
    let config = &state.config;

    // Rescan all roots
    if let Ok(mut result) = crate::scanner::scan_all_roots(&roots, config) {
        let count = result.projects.len();
        // Re-merge user metadata after rescan
        crate::data::merge_user_data(&mut result.projects, &state.data_store);
        state.projects = result.projects;

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

// ─── Scan Roots API ─────────────────────────────────────────

/// List all scan roots
pub async fn get_roots(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let state = state.lock().unwrap();
    let roots: Vec<serde_json::Value> = state
        .roots
        .iter()
        .map(|r| {
            serde_json::json!({
                "path": r.path.to_string_lossy(),
                "label": r.label_or_path(),
                "enabled": r.enabled,
            })
        })
        .collect();

    Json(serde_json::json!({ "roots": roots }))
}

/// Add a new scan root
pub async fn add_root(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let path_str = body.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path_str.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "status": "error", "message": "path is required" })),
        );
    }

    let path = std::path::PathBuf::from(path_str);
    let resolved = if path.is_absolute() {
        path
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(&path),
            Err(_) => path,
        }
    };

    let mut state = state.lock().unwrap();
    if state.roots.iter().any(|r| r.path == resolved) {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "status": "error",
                "message": format!("Root already exists: {}", resolved.display())
            })),
        );
    }

    let root = crate::roots::ScanRoot::new(resolved);
    state.roots.push(root);
    let _ = crate::roots::save_roots(&state.roots);

    // Rescan with new root
    let roots = state.roots.clone();
    let config = &state.config;
    if let Ok(mut result) = crate::scanner::scan_all_roots(&roots, config) {
        crate::data::merge_user_data(&mut result.projects, &state.data_store);
        state.projects = result.projects;
    }

    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

/// Remove a scan root
pub async fn remove_root(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    if index >= state.roots.len() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "status": "error", "message": "Invalid index" })),
        );
    }

    state.roots.remove(index);
    let _ = crate::roots::save_roots(&state.roots);

    // Rescan without the removed root
    let roots = state.roots.clone();
    let config = &state.config;
    if let Ok(mut result) = crate::scanner::scan_all_roots(&roots, config) {
        crate::data::merge_user_data(&mut result.projects, &state.data_store);
        state.projects = result.projects;
    }

    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

/// Update a scan root (toggle enabled / update label)
pub async fn update_root(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    if index >= state.roots.len() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "status": "error", "message": "Invalid index" })),
        );
    }

    if let Some(enabled) = body.get("enabled").and_then(|v| v.as_bool()) {
        state.roots[index].enabled = enabled;
    }
    if let Some(label) = body.get("label").and_then(|v| v.as_str()) {
        state.roots[index].label = Some(label.to_string());
    }

    let _ = crate::roots::save_roots(&state.roots);

    // Rescan
    let roots = state.roots.clone();
    let config = &state.config;
    if let Ok(mut result) = crate::scanner::scan_all_roots(&roots, config) {
        crate::data::merge_user_data(&mut result.projects, &state.data_store);
        state.projects = result.projects;
    }

    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

// ─── User Metadata API ─────────────────────────────────────

/// Update tags for a project
pub async fn update_tags(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    if index >= state.projects.len() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "status": "error", "message": "Invalid index" })),
        );
    }

    let tags: Vec<String> = body
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    state.projects[index].tags = tags.clone();

    // Persist to data store
    let path = state.projects[index].path.to_string_lossy().to_string();
    let meta = crate::data::ProjectMeta {
        tags: tags.clone(),
        note: state.projects[index].note.clone(),
        status: state.projects[index]
            .user_status
            .as_ref()
            .map(|s| s.label().to_string()),
    };
    state.data_store.projects.insert(path, meta);
    let _ = crate::data::save_data(&state.data_store);

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "tags": tags })),
    )
}

/// Update note for a project
pub async fn update_note(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    if index >= state.projects.len() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "status": "error", "message": "Invalid index" })),
        );
    }

    let note = body.get("note").and_then(|v| v.as_str()).map(String::from);
    state.projects[index].note = note.clone();

    // Persist to data store
    let path = state.projects[index].path.to_string_lossy().to_string();
    let meta = crate::data::ProjectMeta {
        tags: state.projects[index].tags.clone(),
        note: note.clone(),
        status: state.projects[index]
            .user_status
            .as_ref()
            .map(|s| s.label().to_string()),
    };
    state.data_store.projects.insert(path, meta);
    let _ = crate::data::save_data(&state.data_store);

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "note": note })),
    )
}

/// Update user status for a project
pub async fn update_status(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    if index >= state.projects.len() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "status": "error", "message": "Invalid index" })),
        );
    }

    let status_str = body.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let user_status = UserStatus::parse(status_str);

    // Update activity level based on status
    if let Some(ref s) = user_status {
        match s {
            UserStatus::Complete => {
                state.projects[index].activity = crate::project::ActivityLevel::Done
            }
            UserStatus::Archived => {
                state.projects[index].activity = crate::project::ActivityLevel::Archived
            }
            UserStatus::Active => {
                state.projects[index].activity = crate::project::ActivityLevel::Active
            }
            _ => {}
        }
    }

    state.projects[index].user_status = user_status.clone();

    // Persist to data store
    let path = state.projects[index].path.to_string_lossy().to_string();
    let meta = crate::data::ProjectMeta {
        tags: state.projects[index].tags.clone(),
        note: state.projects[index].note.clone(),
        status: user_status.as_ref().map(|s| s.label().to_string()),
    };
    state.data_store.projects.insert(path, meta);
    let _ = crate::data::save_data(&state.data_store);

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "user_status": user_status.map(|s| s.label()) })),
    )
}

// ─── Quick Actions API ─────────────────────────────────────

/// Trigger a quick action for a project
pub async fn project_action(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let state = state.lock().unwrap();
    if index >= state.projects.len() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "status": "error", "message": "Invalid index" })),
        );
    }

    let project = &state.projects[index];
    let action = body.get("action").and_then(|v| v.as_str()).unwrap_or("");
    let project_path = project.path.to_string_lossy().to_string();

    match action {
        "editor" => {
            let editor = state.config.editor_cmd();
            let result = StdCommand::new(&editor).arg(&project_path).spawn();
            match result {
                Ok(_) => (
                    StatusCode::OK,
                    Json(
                        serde_json::json!({ "status": "ok", "message": format!("Opened {} in {}", project_path, editor) }),
                    ),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        serde_json::json!({ "status": "error", "message": format!("Failed to open editor: {}", e) }),
                    ),
                ),
            }
        }
        "terminal" => {
            let terminal = state.config.terminal_cmd();
            let cwd_flag = crate::config::Config::terminal_cwd_flag(&terminal);
            let result = StdCommand::new(&terminal)
                .arg(cwd_flag)
                .arg(&project_path)
                .spawn();
            match result {
                Ok(_) => (
                    StatusCode::OK,
                    Json(
                        serde_json::json!({ "status": "ok", "message": format!("Opened terminal for {}", project_path) }),
                    ),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        serde_json::json!({ "status": "error", "message": format!("Failed to open terminal: {}", e) }),
                    ),
                ),
            }
        }
        "file_manager" => {
            let open_result = open::that(&project_path);
            match open_result {
                Ok(_) => (
                    StatusCode::OK,
                    Json(
                        serde_json::json!({ "status": "ok", "message": format!("Opened file manager for {}", project_path) }),
                    ),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        serde_json::json!({ "status": "error", "message": format!("Failed to open file manager: {}", e) }),
                    ),
                ),
            }
        }
        "github" => {
            // Try to open the GitHub remote
            if let Some(git) = &project.git {
                if git.has_remote {
                    // Try to get the remote URL
                    if let Ok(repo) = git2::Repository::open(&project.path) {
                        if let Ok(remote) = repo.find_remote("origin") {
                            if let Some(url) = remote.url() {
                                // Convert git URL to web URL
                                let web_url = url
                                    .replace("git@github.com:", "https://github.com/")
                                    .replace("git@github.com/", "https://github.com/")
                                    .replace(".git", "");
                                let _ = open::that(&web_url);
                                return (
                                    StatusCode::OK,
                                    Json(
                                        serde_json::json!({ "status": "ok", "message": format!("Opened {}", web_url) }),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            (
                StatusCode::BAD_REQUEST,
                Json(
                    serde_json::json!({ "status": "error", "message": "No GitHub remote configured for this project" }),
                ),
            )
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({ "status": "error", "message": format!("Unknown action: {}", action) }),
            ),
        ),
    }
}

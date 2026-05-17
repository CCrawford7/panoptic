use crate::project::GitState;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::Path;

/// Get the git state for a repository
pub fn get_git_state(repo_path: &Path) -> Result<GitState> {
    let mut repo = git2::Repository::open(repo_path)
        .with_context(|| format!("Failed to open git repo at {:?}", repo_path))?;

    // Get current branch name
    let branch = get_current_branch(&repo).unwrap_or_else(|| "HEAD".to_string());

    // Get dirty/change counts
    let (staged, unstaged, untracked) = get_change_counts(&mut repo);

    let is_dirty = staged > 0 || unstaged > 0 || untracked > 0;

    // Get ahead/behind counts
    let (ahead, behind) = get_ahead_behind(&repo).unwrap_or((0, 0));

    // Get last commit info
    let (last_commit_time, last_commit_message, last_commit_author, total_commits) =
        get_last_commit_info(&repo);

    // Get stash count
    let mut stash_count = 0u32;
    let _ = repo.stash_foreach(|_, _, _| {
        stash_count += 1;
        true
    });

    // Check if remote exists
    let has_remote = repo.find_remote("origin").is_ok();

    Ok(GitState {
        branch,
        is_dirty,
        staged,
        unstaged,
        untracked,
        ahead,
        behind,
        last_commit_time,
        last_commit_message,
        last_commit_author,
        total_commits,
        stash_count,
        has_remote,
    })
}

/// Get the current branch name
fn get_current_branch(repo: &git2::Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if head.is_branch() {
        let name = head.shorthand()?;
        Some(name.to_string())
    } else {
        // Detached HEAD - get the commit hash
        let oid = head.target()?;
        Some(format!("detached@{}", &oid.to_string()[..8]))
    }
}

/// Get the counts of staged, unstaged, and untracked changes
fn get_change_counts(repo: &mut git2::Repository) -> (u32, u32, u32) {
    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;

    // Use diff to count changes
    if let Ok(diff) = repo.diff_index_to_workdir(None, None) {
        diff.foreach(
            &mut |_delta, _progress| true,
            None,
            None,
            Some(&mut |_delta, _hunk, _line| {
                unstaged += 1;
                true
            }),
        )
        .ok();
    }

    // Count staged changes
    if let Ok(head_tree) = repo.head().and_then(|h| h.peel_to_tree()) {
        if let Ok(diff) = repo.diff_tree_to_index(Some(&head_tree), None, None) {
            diff.foreach(
                &mut |_delta, _progress| true,
                None,
                None,
                Some(&mut |_delta, _hunk, _line| {
                    staged += 1;
                    true
                }),
            )
            .ok();
        }
    }

    // Count untracked files
    if let Ok(statuses) = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .recurse_untracked_dirs(true),
    )) {
        for entry in statuses.iter() {
            if entry.status() == git2::Status::WT_NEW {
                untracked += 1;
            }
        }
    }

    (staged, unstaged, untracked)
}

/// Get ahead/behind counts vs upstream branch
fn get_ahead_behind(repo: &git2::Repository) -> Result<(u32, u32)> {
    let head = repo.head()?;
    let head_oid = head.target().context("No HEAD target")?;

    // Find upstream branch
    let upstream_ref = if head.is_branch() {
        let branch_name = head.shorthand().context("No branch name")?;
        let refname = format!("refs/remotes/origin/{}", branch_name);
        repo.find_reference(&refname).ok()
    } else {
        None
    };

    if let Some(upstream) = upstream_ref {
        if let Some(upstream_oid) = upstream.target() {
            let (ahead, behind) = repo.graph_ahead_behind(head_oid, upstream_oid)?;
            return Ok((ahead as u32, behind as u32));
        }
    }

    Ok((0, 0))
}

/// Get info about the last commit
fn get_last_commit_info(repo: &git2::Repository) -> (Option<DateTime<Utc>>, Option<String>, Option<String>, u32) {
    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(_) => return (None, None, None, 0),
    };

    if revwalk.push_head().is_err() {
        return (None, None, None, 0);
    }

    let mut count = 0u32;
    let mut last_time: Option<DateTime<Utc>> = None;
    let mut last_msg: Option<String> = None;
    let mut last_author: Option<String> = None;

    for oid in revwalk.flatten() {
        if let Ok(commit) = repo.find_commit(oid) {
            count += 1;

            // First commit is the most recent
            if count == 1 {
                let time = commit.time();
                let timestamp = time.seconds();
                last_time = DateTime::from_timestamp(timestamp, 0);
                last_msg = Some(
                    commit
                        .message()
                        .unwrap_or("")
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string(),
                );
                last_author = Some(
                    commit
                        .author()
                        .name()
                        .unwrap_or("unknown")
                        .to_string(),
                );
            }
        }
    }

    (last_time, last_msg, last_author, count)
}

//! Progressive worktree collection with parallel git operations.
//!
//! This module contains the implementation of cell-by-cell progressive rendering.
//! Git operations run in parallel and send updates as they complete.
//!
//! TODO(error-handling): Current implementation silently swallows git errors
//! and logs warnings to stderr. Consider whether failures should:
//! - Propagate to user (fail-fast)
//! - Show error placeholder in UI
//! - Continue silently (current behavior)

use crossbeam_channel::Sender;
use worktrunk::git::{LineDiff, Repository, Worktree};

use super::ci_status::PrStatus;
use super::collect::{CellUpdate, detect_worktree_state};
use super::model::{AheadBehind, BranchDiffTotals, CommitDetails, UpstreamStatus};

/// Collect worktree data progressively, sending cell updates as each task completes.
///
/// Spawns 9 parallel git operations:
/// 1. Commit details (timestamp, message)
/// 2. Ahead/behind counts
/// 3. Branch diff stats
/// 4. Working tree diff + status symbols
/// 5. Conflicts check
/// 6. Worktree state detection
/// 7. User status from git config
/// 8. Upstream tracking status
/// 9. CI/PR status
///
/// Each task sends a CellUpdate when it completes, enabling progressive UI updates.
/// Errors are handled with TODO for simplicity (simplest thing for now).
pub fn collect_worktree_progressive(
    wt: &Worktree,
    primary: &Worktree,
    item_idx: usize,
    fetch_ci: bool,
    check_conflicts: bool,
    tx: Sender<CellUpdate>,
) {
    let base_branch = primary
        .branch
        .as_deref()
        .filter(|_| wt.path != primary.path);

    // Clone data needed across threads
    let wt_path = wt.path.clone();
    let wt_head = wt.head.clone();
    let wt_branch = wt.branch.clone();
    let base_branch_owned = base_branch.map(String::from);

    // Spawn all tasks in parallel using scoped threads
    std::thread::scope(|s| {
        // Task 1: Commit details
        {
            let tx = tx.clone();
            let head = wt_head.clone();
            let path = wt_path.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors - for now, simplest thing is to skip on error
                if let (Ok(timestamp), Ok(commit_message)) =
                    (repo.commit_timestamp(&head), repo.commit_message(&head))
                {
                    let _ = tx.send(CellUpdate::CommitDetails {
                        item_idx,
                        commit: CommitDetails {
                            timestamp,
                            commit_message,
                        },
                    });
                }
            });
        }

        // Task 2: Ahead/behind counts
        if let Some(base) = base_branch_owned.as_deref() {
            let tx = tx.clone();
            let head = wt_head.clone();
            let path = wt_path.clone();
            let base = base.to_string();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors
                if let Ok((ahead, behind)) = repo.ahead_behind(&base, &head) {
                    let _ = tx.send(CellUpdate::AheadBehind {
                        item_idx,
                        counts: AheadBehind { ahead, behind },
                    });
                }
            });
        }

        // Task 3: Branch diff
        if let Some(base) = base_branch_owned.as_deref() {
            let tx = tx.clone();
            let head = wt_head.clone();
            let path = wt_path.clone();
            let base = base.to_string();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors
                if let Ok(diff) = repo.branch_diff_stats(&base, &head) {
                    let _ = tx.send(CellUpdate::BranchDiff {
                        item_idx,
                        branch_diff: BranchDiffTotals { diff },
                    });
                }
            });
        }

        // Task 4: Working tree diff + status symbols
        {
            let tx = tx.clone();
            let path = wt_path.clone();
            let base = base_branch_owned.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors
                if let Ok(status_output) = repo.run_command(&["status", "--porcelain"]) {
                    // Parse status to get symbols and is_dirty
                    let (working_tree_symbols, is_dirty) = parse_status_for_symbols(&status_output);

                    // Get working tree diff
                    let working_tree_diff = if is_dirty {
                        repo.working_tree_diff_stats().unwrap_or_default()
                    } else {
                        LineDiff::default()
                    };

                    // Get diff with main
                    let working_tree_diff_with_main = repo
                        .working_tree_diff_with_base(base.as_deref(), is_dirty)
                        .ok()
                        .flatten();

                    let _ = tx.send(CellUpdate::WorkingTreeDiff {
                        item_idx,
                        working_tree_diff,
                        working_tree_diff_with_main,
                        working_tree_symbols,
                        is_dirty,
                    });
                }
            });
        }

        // Task 5: Conflicts check (always send, even if not checking)
        {
            let tx = tx.clone();
            if check_conflicts && let Some(base) = base_branch_owned.as_deref() {
                let head = wt_head.clone();
                let path = wt_path.clone();
                let base = base.to_string();
                s.spawn(move || {
                    let repo = Repository::at(&path);
                    // TODO: Handle errors
                    let has_conflicts = repo.has_merge_conflicts(&base, &head).unwrap_or(false);
                    let _ = tx.send(CellUpdate::Conflicts {
                        item_idx,
                        has_conflicts,
                    });
                });
            } else {
                // Send default value when not checking conflicts
                let _ = tx.send(CellUpdate::Conflicts {
                    item_idx,
                    has_conflicts: false,
                });
            }
        }

        // Task 6: Worktree state detection
        {
            let tx = tx.clone();
            let path = wt_path.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                let worktree_state = detect_worktree_state(&repo);
                let _ = tx.send(CellUpdate::WorktreeState {
                    item_idx,
                    worktree_state,
                });
            });
        }

        // Task 7: User status
        {
            let tx = tx.clone();
            let path = wt_path.clone();
            let branch = wt_branch.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                let user_status = repo.user_status(branch.as_deref());
                let _ = tx.send(CellUpdate::UserStatus {
                    item_idx,
                    user_status,
                });
            });
        }

        // Task 8: Upstream status (always sends)
        {
            let tx = tx.clone();
            let branch = wt_branch.clone();
            let head = wt_head.clone();
            let path = wt_path.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                let upstream = if let Some(branch) = branch.as_deref() {
                    match repo.upstream_branch(branch) {
                        Ok(Some(upstream_branch)) => {
                            let remote =
                                upstream_branch.split_once('/').map(|(r, _)| r.to_string());
                            match repo.ahead_behind(&upstream_branch, &head) {
                                Ok((ahead, behind)) => Some(UpstreamStatus {
                                    remote,
                                    ahead,
                                    behind,
                                }),
                                Err(e) => {
                                    eprintln!(
                                        "Warning: ahead_behind failed for {}: {}",
                                        path.display(),
                                        e
                                    );
                                    None
                                }
                            }
                        }
                        Ok(None) => None, // No upstream configured
                        Err(e) => {
                            eprintln!(
                                "Warning: upstream_branch failed for {}: {}",
                                path.display(),
                                e
                            );
                            None
                        }
                    }
                } else {
                    None // No branch (detached HEAD)
                };
                let upstream = upstream.unwrap_or_default();
                let _ = tx.send(CellUpdate::Upstream { item_idx, upstream });
            });
        }

        // Task 9: CI status
        if fetch_ci {
            let tx = tx.clone();
            let branch = wt_branch.clone();
            let head = wt_head.clone();
            let path = wt_path.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors
                if let Ok(repo_path) = repo.worktree_root() {
                    let pr_status = branch
                        .as_deref()
                        .and_then(|branch| PrStatus::detect(branch, &head, &repo_path));
                    let _ = tx.send(CellUpdate::CiStatus {
                        item_idx,
                        pr_status,
                    });
                }
            });
        }
    });
}

/// Parse git status output to extract working tree symbols.
/// Returns (symbols, is_dirty).
fn parse_status_for_symbols(status_output: &str) -> (String, bool) {
    let mut has_untracked = false;
    let mut has_modified = false;
    let mut has_staged = false;
    let mut has_renamed = false;
    let mut has_deleted = false;
    let mut is_dirty = false;

    for line in status_output.lines() {
        if line.len() < 2 {
            continue;
        }

        is_dirty = true;

        let bytes = line.as_bytes();
        let index_status = bytes[0] as char;
        let worktree_status = bytes[1] as char;

        if index_status == '?' && worktree_status == '?' {
            has_untracked = true;
        }

        if worktree_status == 'M' {
            has_modified = true;
        }

        if index_status == 'A' || index_status == 'M' || index_status == 'C' {
            has_staged = true;
        }

        if index_status == 'R' {
            has_renamed = true;
        }

        if index_status == 'D' || worktree_status == 'D' {
            has_deleted = true;
        }
    }

    // Build working tree string
    let mut working_tree = String::new();
    if has_untracked {
        working_tree.push('?');
    }
    if has_modified {
        working_tree.push('!');
    }
    if has_staged {
        working_tree.push('+');
    }
    if has_renamed {
        working_tree.push('»');
    }
    if has_deleted {
        working_tree.push('✘');
    }

    (working_tree, is_dirty)
}

/// Collect branch data progressively, sending cell updates as each task completes.
///
/// Spawns 6 parallel git operations (similar to worktrees but without working tree operations):
/// 1. Commit details (timestamp, message)
/// 2. Ahead/behind counts
/// 3. Branch diff stats
/// 4. Upstream tracking status
/// 5. Conflicts check
/// 6. CI/PR status
pub fn collect_branch_progressive(
    branch_name: &str,
    commit_sha: &str,
    primary: &Worktree,
    item_idx: usize,
    fetch_ci: bool,
    check_conflicts: bool,
    tx: Sender<CellUpdate>,
) {
    let base_branch = primary.branch.as_deref();
    let repo_path = primary.path.clone();

    // Clone data needed across threads
    let branch_name_owned = branch_name.to_string();
    let commit_sha_owned = commit_sha.to_string();

    // Spawn all tasks in parallel using scoped threads
    std::thread::scope(|s| {
        // Task 1: Commit details
        {
            let tx = tx.clone();
            let sha = commit_sha_owned.clone();
            let path = repo_path.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors - for now, simplest thing is to skip on error
                if let (Ok(timestamp), Ok(commit_message)) =
                    (repo.commit_timestamp(&sha), repo.commit_message(&sha))
                {
                    let _ = tx.send(CellUpdate::CommitDetails {
                        item_idx,
                        commit: CommitDetails {
                            timestamp,
                            commit_message,
                        },
                    });
                }
            });
        }

        // Task 2: Ahead/behind counts
        if let Some(base) = base_branch {
            let tx = tx.clone();
            let sha = commit_sha_owned.clone();
            let path = repo_path.clone();
            let base = base.to_string();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors
                if let Ok((ahead, behind)) = repo.ahead_behind(&base, &sha) {
                    let _ = tx.send(CellUpdate::AheadBehind {
                        item_idx,
                        counts: AheadBehind { ahead, behind },
                    });
                }
            });
        }

        // Task 3: Branch diff
        if let Some(base) = base_branch {
            let tx = tx.clone();
            let sha = commit_sha_owned.clone();
            let path = repo_path.clone();
            let base = base.to_string();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors
                if let Ok(diff) = repo.branch_diff_stats(&base, &sha) {
                    let _ = tx.send(CellUpdate::BranchDiff {
                        item_idx,
                        branch_diff: BranchDiffTotals { diff },
                    });
                }
            });
        }

        // Task 4: Upstream status
        {
            let tx = tx.clone();
            let branch = branch_name_owned.clone();
            let sha = commit_sha_owned.clone();
            let path = repo_path.clone();
            s.spawn(move || {
                let repo = Repository::at(&path);
                let upstream = match repo.upstream_branch(&branch) {
                    Ok(Some(upstream_branch)) => {
                        let remote = upstream_branch.split_once('/').map(|(r, _)| r.to_string());
                        match repo.ahead_behind(&upstream_branch, &sha) {
                            Ok((ahead, behind)) => Some(UpstreamStatus {
                                remote,
                                ahead,
                                behind,
                            }),
                            Err(_) => None,
                        }
                    }
                    Ok(None) => None, // No upstream configured
                    Err(_) => None,
                };

                let _ = tx.send(CellUpdate::Upstream {
                    item_idx,
                    upstream: upstream.unwrap_or(UpstreamStatus {
                        remote: None,
                        ahead: 0,
                        behind: 0,
                    }),
                });
            });
        }

        // Task 5: Conflicts check
        if check_conflicts && let Some(base) = base_branch {
            let tx = tx.clone();
            let sha = commit_sha_owned.clone();
            let path = repo_path.clone();
            let base = base.to_string();
            s.spawn(move || {
                let repo = Repository::at(&path);
                // TODO: Handle errors
                if let Ok(has_conflicts) = repo.has_merge_conflicts(&base, &sha) {
                    let _ = tx.send(CellUpdate::Conflicts {
                        item_idx,
                        has_conflicts,
                    });
                }
            });
        }

        // Task 6: CI/PR status
        if fetch_ci {
            let tx = tx.clone();
            let branch = branch_name_owned.clone();
            let sha = commit_sha_owned.clone();
            let path = repo_path.clone();
            s.spawn(move || {
                let pr_status = PrStatus::detect(&branch, &sha, &path);
                let _ = tx.send(CellUpdate::CiStatus {
                    item_idx,
                    pr_status,
                });
            });
        }
    });
}

mod layout;
mod render;

use rayon::prelude::*;
use std::path::Path;
use worktrunk::git::{
    GitError, get_ahead_behind_in, get_branch_diff_stats_in, get_commit_message_in,
    get_commit_timestamp_in, get_upstream_branch_in, get_working_tree_diff_stats_in,
    get_worktree_root_in, get_worktree_state_in, list_worktrees,
};

use layout::calculate_responsive_layout;
use render::{format_header_line, format_worktree_line};

pub struct WorktreeInfo {
    pub path: std::path::PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub timestamp: i64,
    pub commit_message: String,
    pub ahead: usize,
    pub behind: usize,
    pub working_tree_diff: (usize, usize),
    pub branch_diff: (usize, usize),
    pub is_primary: bool,
    pub is_current: bool,
    pub detached: bool,
    pub bare: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
    pub upstream_remote: Option<String>,
    pub upstream_ahead: usize,
    pub upstream_behind: usize,
    pub worktree_state: Option<String>,
}

pub fn handle_list() -> Result<(), GitError> {
    let worktrees = list_worktrees()?;

    if worktrees.is_empty() {
        return Ok(());
    }

    // First worktree is the primary
    let primary = &worktrees[0];
    let primary_branch = primary.branch.as_ref();

    // Get current worktree to identify active one
    let current_worktree_path = get_worktree_root_in(Path::new(".")).ok();

    // Helper function to process a single worktree
    let process_worktree = |idx: usize, wt: &worktrunk::git::Worktree| -> WorktreeInfo {
        let is_primary = idx == 0;
        let is_current = current_worktree_path
            .as_ref()
            .map(|p| p == &wt.path)
            .unwrap_or(false);

        // Get commit timestamp
        let timestamp = get_commit_timestamp_in(&wt.path, &wt.head).unwrap_or(0);

        // Get commit message
        let commit_message = get_commit_message_in(&wt.path, &wt.head).unwrap_or_default();

        // Calculate ahead/behind relative to primary branch (only if primary has a branch)
        let (ahead, behind) = if is_primary {
            (0, 0)
        } else if let Some(pb) = primary_branch {
            get_ahead_behind_in(&wt.path, pb, &wt.head).unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        let working_tree_diff = get_working_tree_diff_stats_in(&wt.path).unwrap_or((0, 0));

        // Get branch diff stats (downstream of primary, only if primary has a branch)
        let branch_diff = if is_primary {
            (0, 0)
        } else if let Some(pb) = primary_branch {
            get_branch_diff_stats_in(&wt.path, pb, &wt.head).unwrap_or((0, 0))
        } else {
            (0, 0)
        };

        // Get upstream tracking info
        let (upstream_remote, upstream_ahead, upstream_behind) = if let Some(ref branch) = wt.branch
        {
            if let Ok(Some(upstream_branch)) = get_upstream_branch_in(&wt.path, branch) {
                // Extract remote name from "origin/main" -> "origin"
                let remote = upstream_branch
                    .split('/')
                    .next()
                    .unwrap_or("origin")
                    .to_string();
                let (ahead, behind) =
                    get_ahead_behind_in(&wt.path, &upstream_branch, &wt.head).unwrap_or((0, 0));
                (Some(remote), ahead, behind)
            } else {
                (None, 0, 0)
            }
        } else {
            (None, 0, 0)
        };

        // Get worktree state (merge/rebase/etc)
        let worktree_state = get_worktree_state_in(&wt.path).unwrap_or(None);

        WorktreeInfo {
            path: wt.path.clone(),
            head: wt.head.clone(),
            branch: wt.branch.clone(),
            timestamp,
            commit_message,
            ahead,
            behind,
            working_tree_diff,
            branch_diff,
            is_primary,
            is_current,
            detached: wt.detached,
            bare: wt.bare,
            locked: wt.locked.clone(),
            prunable: wt.prunable.clone(),
            upstream_remote,
            upstream_ahead,
            upstream_behind,
            worktree_state,
        }
    };

    // Gather enhanced information for all worktrees in parallel
    //
    // Parallelization strategy: Use Rayon to process worktrees concurrently.
    // Each worktree requires ~5 git operations (timestamp, ahead/behind, diffs).
    //
    // Benchmark results: See benches/list.rs for sequential vs parallel comparison.
    //
    // Decision: Always use parallel for simplicity and 2+ worktree performance.
    // Rayon overhead (~1-2ms) is acceptable for single-worktree case.
    //
    // TODO: Could parallelize the 5 git commands within each worktree if needed,
    // but worktree-level parallelism provides the best cost/benefit tradeoff
    let mut infos: Vec<WorktreeInfo> = if std::env::var("WT_SEQUENTIAL").is_ok() {
        // Sequential iteration (for benchmarking)
        worktrees
            .iter()
            .enumerate()
            .map(|(idx, wt)| process_worktree(idx, wt))
            .collect()
    } else {
        // Parallel iteration (default)
        worktrees
            .par_iter()
            .enumerate()
            .map(|(idx, wt)| process_worktree(idx, wt))
            .collect()
    };

    // Sort by most recent commit (descending)
    infos.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Calculate responsive layout based on terminal width
    let layout = calculate_responsive_layout(&infos);

    // Display header
    format_header_line(&layout);

    // Display formatted output
    for info in &infos {
        format_worktree_line(info, &layout);
    }

    Ok(())
}

//! Worktree data collection with parallelized git operations.
//!
//! This module provides an efficient approach to collecting worktree data:
//! - Parallel collection across worktrees (using Rayon)
//! - Parallel operations within each worktree (using scoped threads)
//! - Progressive updates via channels (update UI as each worktree completes)
//!
//! ## Unified Collection Architecture
//!
//! Both progressive (with progress bars) and buffered (silent) modes use the same
//! `collect_internal()` function. The only difference is whether progress bars are
//! created and shown. This ensures a single canonical collection implementation.
//!
//! **Parallelism at two levels**:
//! - Across worktrees: Multiple worktrees collected concurrently via Rayon
//! - Within worktrees: Git operations (ahead/behind, diffs, CI) run concurrently via scoped threads
//!
//! This ensures fast operations don't wait for slow ones (e.g., CI doesn't block ahead/behind counts)
use crossbeam_channel as chan;
use rayon::prelude::*;
use worktrunk::git::{GitError, LineDiff, Repository, Worktree};
use worktrunk::styling::INFO_EMOJI;

use super::ci_status::PrStatus;
use super::model::{
    AheadBehind, BranchDiffTotals, BranchState, CommitDetails, GitOperation, ItemKind, ListItem,
    MainDivergence, StatusSymbols, UpstreamDivergence, UpstreamStatus,
};

/// Cell update messages sent as each git operation completes.
/// These enable progressive rendering - update UI as data arrives.
#[derive(Debug, Clone)]
pub(super) enum CellUpdate {
    /// Commit timestamp and message
    CommitDetails {
        item_idx: usize,
        commit: CommitDetails,
    },
    /// Ahead/behind counts vs main
    AheadBehind {
        item_idx: usize,
        counts: AheadBehind,
    },
    /// Line diff vs main branch
    BranchDiff {
        item_idx: usize,
        branch_diff: BranchDiffTotals,
    },
    /// Working tree diff and symbols (?, !, +, », ✘)
    WorkingTreeDiff {
        item_idx: usize,
        working_tree_diff: LineDiff,
        working_tree_diff_with_main: Option<LineDiff>,
        /// Symbols for uncommitted changes (?, !, +, », ✘)
        working_tree_symbols: String,
        is_dirty: bool,
    },
    /// Merge conflicts with main
    Conflicts {
        item_idx: usize,
        has_conflicts: bool,
    },
    /// Git operation in progress (rebase/merge)
    WorktreeState {
        item_idx: usize,
        worktree_state: Option<String>,
    },
    /// User-defined status from git config
    UserStatus {
        item_idx: usize,
        user_status: Option<String>,
    },
    /// Upstream tracking status
    Upstream {
        item_idx: usize,
        upstream: UpstreamStatus,
    },
    /// CI/PR status (slow operation)
    CiStatus {
        item_idx: usize,
        pr_status: Option<PrStatus>,
    },
}

impl CellUpdate {
    /// Get the item index for this update
    fn item_idx(&self) -> usize {
        match self {
            CellUpdate::CommitDetails { item_idx, .. } => *item_idx,
            CellUpdate::AheadBehind { item_idx, .. } => *item_idx,
            CellUpdate::BranchDiff { item_idx, .. } => *item_idx,
            CellUpdate::WorkingTreeDiff { item_idx, .. } => *item_idx,
            CellUpdate::Conflicts { item_idx, .. } => *item_idx,
            CellUpdate::WorktreeState { item_idx, .. } => *item_idx,
            CellUpdate::UserStatus { item_idx, .. } => *item_idx,
            CellUpdate::Upstream { item_idx, .. } => *item_idx,
            CellUpdate::CiStatus { item_idx, .. } => *item_idx,
        }
    }
}

/// Detect if a worktree is in the middle of a git operation (rebase/merge).
pub(super) fn detect_worktree_state(repo: &Repository) -> Option<String> {
    let git_dir = repo.git_dir().ok()?;

    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        Some("rebase".to_string())
    } else if git_dir.join("MERGE_HEAD").exists() {
        Some("merge".to_string())
    } else {
        None
    }
}

/// Compute status symbols from individual components (for progressive rendering).
///
/// This is a simplified version of compute_status_symbols that works with individual fields
/// rather than StatusInfo struct. Used during progressive rendering when fields arrive separately.
#[allow(clippy::too_many_arguments)]
fn compute_status_symbols_from_parts(
    working_tree_symbols: &str,
    counts: &AheadBehind,
    upstream: &UpstreamStatus,
    has_conflicts: bool,
    worktree_state: Option<&str>,
    wt: &Worktree,
    working_tree_diff: &LineDiff,
    working_tree_diff_with_main: Option<&LineDiff>,
    is_primary: bool,
    base_branch: Option<&str>,
    user_status: Option<String>,
) -> StatusSymbols {
    // Build main divergence
    let main_divergence = match (counts.ahead, counts.behind) {
        (0, 0) => MainDivergence::None,
        (a, 0) if a > 0 => MainDivergence::Ahead,
        (0, b) if b > 0 => MainDivergence::Behind,
        _ => MainDivergence::Diverged,
    };

    // Build upstream divergence
    let (upstream_ahead, upstream_behind) =
        upstream.active().map(|(_, a, b)| (a, b)).unwrap_or((0, 0));
    let upstream_divergence = match (upstream_ahead, upstream_behind) {
        (0, 0) => UpstreamDivergence::None,
        (a, 0) if a > 0 => UpstreamDivergence::Ahead,
        (0, b) if b > 0 => UpstreamDivergence::Behind,
        _ => UpstreamDivergence::Diverged,
    };

    // Determine branch state (only for non-primary worktrees with base branch)
    let branch_state = if !is_primary && base_branch.is_some() {
        // Check for MatchesMain (requires mdiff to confirm working tree matches main's tree)
        if let Some(mdiff) = working_tree_diff_with_main {
            if mdiff.added == 0 && mdiff.deleted == 0 && counts.ahead == 0 {
                BranchState::MatchesMain
            } else if counts.ahead == 0
                && working_tree_diff.added == 0
                && working_tree_diff.deleted == 0
            {
                BranchState::NoCommits
            } else {
                BranchState::None
            }
        } else {
            // mdiff is None (optimization when trees differ)
            // Can still determine NoCommits without computing diff
            if counts.ahead == 0 && working_tree_diff.added == 0 && working_tree_diff.deleted == 0 {
                BranchState::NoCommits
            } else {
                BranchState::None
            }
        }
    } else {
        BranchState::None
    };

    // Determine git operation
    let git_operation = match worktree_state {
        Some("rebase") => GitOperation::Rebase,
        Some("merge") => GitOperation::Merge,
        _ => GitOperation::None,
    };

    // Worktree attributes
    let mut worktree_attrs = String::new();
    // Note: wt.bare is always false here because WorktreeList filters out bare worktrees
    // See src/git/mod.rs:88 - bare worktrees are removed before reaching this code
    if wt.locked.is_some() {
        worktree_attrs.push('⊠');
    }
    if wt.prunable.is_some() {
        worktree_attrs.push('⚠');
    }

    StatusSymbols {
        has_conflicts,
        has_potential_conflicts: false, // Not computed in fast path
        branch_state,
        git_operation,
        worktree_attrs,
        main_divergence,
        upstream_divergence,
        working_tree: working_tree_symbols.to_string(),
        user_status,
    }
}

/// Compute status_symbols for all worktrees after data collection is complete.
/// Compute status symbols for a single item (worktree only).
/// Returns true if status was computed, false if item is not a worktree or already has status.
fn compute_item_status_symbols(
    item: &mut ListItem,
    sorted_worktrees: &[Worktree],
    primary: &Worktree,
    worktree_idx: usize,
) -> bool {
    // Skip if already computed
    if item.status_symbols.is_some() {
        return false;
    }

    if let ItemKind::Worktree(data) = &item.kind {
        let wt = &sorted_worktrees[worktree_idx];
        let base_branch = primary
            .branch
            .as_deref()
            .filter(|_| wt.path != primary.path);

        item.status_symbols = Some(compute_status_symbols_from_parts(
            data.working_tree_symbols.as_deref().unwrap_or(""),
            item.counts.as_ref().unwrap_or(&AheadBehind::default()),
            item.upstream.as_ref().unwrap_or(&UpstreamStatus::default()),
            item.has_conflicts.unwrap_or(false),
            data.worktree_state.as_deref(),
            wt,
            data.working_tree_diff
                .as_ref()
                .unwrap_or(&LineDiff::default()),
            data.working_tree_diff_with_main
                .as_ref()
                .and_then(|opt| opt.as_ref()),
            data.is_primary,
            base_branch,
            item.user_status.clone(),
        ));
        true
    } else {
        false
    }
}

fn compute_all_status_symbols(
    all_items: &mut [ListItem],
    sorted_worktrees: &[Worktree],
    primary: &Worktree,
) {
    let mut worktree_idx = 0;
    for item in all_items.iter_mut() {
        if compute_item_status_symbols(item, sorted_worktrees, primary, worktree_idx) {
            worktree_idx += 1;
        }
    }
}

/// Drain cell updates from the channel and apply them to worktree_items.
///
/// This is the shared logic between progressive and buffered collection modes.
/// The `on_update` callback is called after each update is processed with the
/// item index and a reference to the updated info, allowing progressive mode
/// to update progress bars while buffered mode does nothing.
fn drain_cell_updates(
    rx: chan::Receiver<CellUpdate>,
    worktree_items: &mut [ListItem],
    mut on_update: impl FnMut(usize, &mut ListItem),
) {
    // Process cell updates as they arrive
    while let Ok(update) = rx.recv() {
        let item_idx = update.item_idx();

        match update {
            CellUpdate::CommitDetails { item_idx, commit } => {
                worktree_items[item_idx].commit = Some(commit);
            }
            CellUpdate::AheadBehind { item_idx, counts } => {
                worktree_items[item_idx].counts = Some(counts);
            }
            CellUpdate::BranchDiff {
                item_idx,
                branch_diff,
            } => {
                worktree_items[item_idx].branch_diff = Some(branch_diff);
            }
            CellUpdate::WorkingTreeDiff {
                item_idx,
                working_tree_diff,
                working_tree_diff_with_main,
                working_tree_symbols,
                is_dirty,
            } => {
                if let ItemKind::Worktree(data) = &mut worktree_items[item_idx].kind {
                    data.working_tree_diff = Some(working_tree_diff);
                    data.working_tree_diff_with_main = Some(working_tree_diff_with_main);
                    data.working_tree_symbols = Some(working_tree_symbols);
                    data.is_dirty = Some(is_dirty);
                }
            }
            CellUpdate::Conflicts {
                item_idx,
                has_conflicts,
            } => {
                worktree_items[item_idx].has_conflicts = Some(has_conflicts);
            }
            CellUpdate::WorktreeState {
                item_idx,
                worktree_state,
            } => {
                if let ItemKind::Worktree(data) = &mut worktree_items[item_idx].kind {
                    data.worktree_state = worktree_state;
                }
            }
            CellUpdate::UserStatus {
                item_idx,
                user_status,
            } => {
                worktree_items[item_idx].user_status = user_status;
            }
            CellUpdate::Upstream { item_idx, upstream } => {
                worktree_items[item_idx].upstream = Some(upstream);
            }
            CellUpdate::CiStatus {
                item_idx,
                pr_status,
            } => {
                // Wrap in Some() to indicate "loaded" (Some(None) = no CI, Some(Some(status)) = has CI)
                worktree_items[item_idx].pr_status = Some(pr_status);
            }
        }

        // Invoke rendering callback (progressive mode re-renders rows, buffered mode does nothing)
        on_update(item_idx, &mut worktree_items[item_idx]);
    }
}

/// Get branches that don't have worktrees.
///
/// Returns (branch_name, commit_sha) pairs for all branches without associated worktrees.
fn get_branches_without_worktrees(
    repo: &Repository,
    worktrees: &[Worktree],
) -> Result<Vec<(String, String)>, GitError> {
    // Get all local branches
    let all_branches = repo.list_local_branches()?;

    // Build a set of branch names that have worktrees
    let worktree_branches: std::collections::HashSet<String> = worktrees
        .iter()
        .filter_map(|wt| wt.branch.clone())
        .collect();

    // Filter to branches without worktrees
    let branches_without_worktrees: Vec<_> = all_branches
        .into_iter()
        .filter(|(branch_name, _)| !worktree_branches.contains(branch_name))
        .collect();

    Ok(branches_without_worktrees)
}

/// Collect worktree data with optional progressive rendering.
///
/// When `show_progress` is true, renders a skeleton immediately and updates as data arrives.
/// When false, silently collects all data and returns it for external rendering.
pub fn collect(
    repo: &Repository,
    show_branches: bool,
    show_full: bool,
    fetch_ci: bool,
    check_conflicts: bool,
    show_progress: bool,
) -> Result<Option<super::model::ListData>, GitError> {
    use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
    use std::time::Duration;

    let worktrees = repo.list_worktrees()?;
    if worktrees.worktrees.is_empty() {
        return Ok(None);
    }

    let primary = worktrees.worktrees[0].clone();
    let current_worktree_path = repo.worktree_root().ok();

    // Sort worktrees for display order
    let sorted_worktrees = sort_worktrees(
        &worktrees.worktrees,
        &primary,
        current_worktree_path.as_ref(),
    );

    // Get branches early for layout calculation and skeleton creation (when --branches is used)
    let branches_without_worktrees = if show_branches {
        get_branches_without_worktrees(repo, &worktrees.worktrees)?
    } else {
        Vec::new()
    };

    let branch_names: Vec<String> = branches_without_worktrees
        .iter()
        .map(|(name, _sha)| name.clone())
        .collect();

    // Calculate layout from basic worktree info + branch names
    let layout = super::layout::calculate_layout_from_basics(
        &sorted_worktrees,
        &branch_names,
        show_full,
        fetch_ci,
    );

    // Single-line invariant: compute width from the draw target (stderr) with right guard
    // to prevent line wrapping (which breaks indicatif's line-based cursor math).
    // Keep 2 columns free: off-by-one terminal behavior + emoji width safety margin.
    let (_, term_cols) = console::Term::stderr().size();
    let max_width = term_cols.saturating_sub(2) as usize;

    let clamp = |s: &str| -> String {
        if console::measure_text_width(s) > max_width {
            console::truncate_str(s, max_width, "…").into_owned()
        } else {
            s.to_owned()
        }
    };

    // Create MultiProgress with explicit draw target and cursor mode
    // Use stderr for progress bars so they don't interfere with stdout directives
    let multi = if show_progress {
        let mp = MultiProgress::with_draw_target(indicatif::ProgressDrawTarget::stderr_with_hz(10));
        mp.set_move_cursor(true); // Stable since bar count is fixed
        mp
    } else {
        MultiProgress::new()
    };

    let message_style = ProgressStyle::with_template("{msg}").unwrap();

    // Create header progress bar (part of transient UI, cleared with finish_and_clear)
    let header_pb = if show_progress {
        let pb = multi.add(ProgressBar::hidden());
        pb.set_style(message_style.clone());
        pb.set_message(clamp(&layout.format_header_line()));
        Some(pb)
    } else {
        None
    };

    // Initialize worktree items with identity fields and None for computed fields
    let mut all_items: Vec<super::model::ListItem> = sorted_worktrees
        .iter()
        .map(|wt| super::model::ListItem {
            // Common fields
            head: wt.head.clone(),
            branch: wt.branch.clone(),
            commit: None,
            counts: None,
            branch_diff: None,
            upstream: None,
            pr_status: None,
            has_conflicts: None,
            user_status: None,
            status_symbols: None,
            display: super::model::DisplayFields::default(),
            // Type-specific data
            kind: super::model::ItemKind::Worktree(Box::new(
                super::model::WorktreeData::from_worktree(wt, wt.path == primary.path),
            )),
        })
        .collect();

    // Initialize branch items with identity fields and None for computed fields
    let branch_start_idx = all_items.len();
    for (branch_name, commit_sha) in &branches_without_worktrees {
        all_items.push(super::model::ListItem {
            // Common fields
            head: commit_sha.clone(),
            branch: Some(branch_name.clone()),
            commit: None,
            counts: None,
            branch_diff: None,
            upstream: None,
            pr_status: None,
            has_conflicts: None,
            user_status: None,
            status_symbols: None,
            display: super::model::DisplayFields::default(),
            // Type-specific data
            kind: super::model::ItemKind::Branch,
        });
    }

    // Create progress bars for all items (worktrees + branches)
    let progress_bars: Vec<_> = all_items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let pb = multi.add(ProgressBar::new_spinner());
            if show_progress {
                pb.set_style(message_style.clone());

                // Render skeleton immediately with clamping
                let skeleton = if idx < sorted_worktrees.len() {
                    // Worktree skeleton
                    let wt = &sorted_worktrees[idx];
                    let is_primary = wt.path == primary.path;
                    let is_current = current_worktree_path
                        .as_ref()
                        .map(|cp| &wt.path == cp)
                        .unwrap_or(false);
                    layout.format_skeleton_row(wt, is_primary, is_current)
                } else {
                    // Branch skeleton
                    layout.format_list_item_line(item, current_worktree_path.as_ref())
                };
                pb.set_message(clamp(&skeleton));
                pb.enable_steady_tick(Duration::from_millis(100));
            }
            pb
        })
        .collect();

    // Cache last rendered message per row to avoid redundant updates
    let mut last_lines: Vec<String> = vec![String::new(); all_items.len()];

    // Footer progress bar with loading status
    // Uses determinate bar (no spinner) with {wide_msg} to prevent clearing artifacts
    let total_cells = all_items.len() * layout.columns.len();
    let num_worktrees = sorted_worktrees.len();
    let num_branches = branches_without_worktrees.len();
    let footer_base = if show_branches && num_branches > 0 {
        format!(
            "Showing {} worktrees, {} branches",
            num_worktrees, num_branches
        )
    } else {
        let plural = if num_worktrees == 1 { "" } else { "s" };
        format!("Showing {} worktree{}", num_worktrees, plural)
    };

    // Spacer: single-line blank between the table rows and the footer (no multiline messages)
    let spacer_style = ProgressStyle::with_template("{wide_msg}").unwrap();
    let spacer_pb = if show_progress {
        let gap = multi.add(ProgressBar::new(1));
        gap.set_style(spacer_style.clone());
        gap.set_message(" "); // padded blank line
        Some(gap)
    } else {
        None
    };

    // Footer is single-line; no '\n'. Will be replaced with final summary on finish.
    let footer_style = ProgressStyle::with_template("{wide_msg}").unwrap();

    let footer_pb = if show_progress {
        use anstyle::Style;
        let dim = Style::new().dimmed();

        // Footer with determinate bar (no spinner/tick)
        let footer = multi.add(ProgressBar::new(total_cells as u64));
        footer.set_style(footer_style);
        footer.set_message(format!(
            "{INFO_EMOJI} {dim}{footer_base} (0/{total_cells} cells loaded){dim:#}"
        ));
        Some(footer)
    } else {
        None
    };

    // Create channel for cell updates
    let (tx, rx) = chan::unbounded();

    // Spawn worktree collection in background thread
    let sorted_worktrees_clone = sorted_worktrees.clone();
    let primary_clone = primary.clone();
    let tx_worktrees = tx.clone();
    std::thread::spawn(move || {
        sorted_worktrees_clone
            .par_iter()
            .enumerate()
            .for_each(|(idx, wt)| {
                super::collect_progressive_impl::collect_worktree_progressive(
                    wt,
                    &primary_clone,
                    idx,
                    fetch_ci,
                    check_conflicts,
                    tx_worktrees.clone(),
                );
            });
    });

    // Spawn branch collection in background thread (if requested)
    if show_branches {
        let branches_clone = branches_without_worktrees.clone();
        let primary_clone = primary.clone();
        let tx_branches = tx.clone();
        std::thread::spawn(move || {
            branches_clone
                .par_iter()
                .enumerate()
                .for_each(|(idx, (branch_name, commit_sha))| {
                    let item_idx = branch_start_idx + idx;
                    super::collect_progressive_impl::collect_branch_progressive(
                        branch_name,
                        commit_sha,
                        &primary_clone,
                        item_idx,
                        fetch_ci,
                        check_conflicts,
                        tx_branches.clone(),
                    );
                });
        });
    }

    // Drop the original sender so drain_cell_updates knows when all spawned threads are done
    drop(tx);

    // Track completed cells for footer progress and worktree index for status computation
    let mut completed_cells = 0;
    let mut worktree_idx_map = vec![None; all_items.len()];
    {
        let mut wt_idx = 0;
        for (item_idx, item) in all_items.iter().enumerate() {
            if matches!(item.kind, ItemKind::Worktree(_)) {
                worktree_idx_map[item_idx] = Some(wt_idx);
                wt_idx += 1;
            }
        }
    }

    // Drain cell updates with conditional progressive rendering
    drain_cell_updates(rx, &mut all_items, |item_idx, info| {
        if show_progress {
            use anstyle::Style;
            let dim = Style::new().dimmed();

            completed_cells += 1;

            // Compute status symbols progressively once we have working_tree_symbols
            // (that indicates we have the core data needed for status computation)
            if info.status_symbols.is_none()
                && let (ItemKind::Worktree(data), Some(wt_idx)) =
                    (&info.kind, worktree_idx_map[item_idx])
                && data.working_tree_symbols.is_some()
            {
                compute_item_status_symbols(info, &sorted_worktrees, &primary, wt_idx);
            }

            // Update footer progress
            if let Some(pb) = footer_pb.as_ref() {
                pb.set_position(completed_cells as u64);
                pb.set_message(format!(
                    "{INFO_EMOJI} {dim}{footer_base} ({completed_cells}/{total_cells} cells loaded){dim:#}"
                ));
            }

            // Re-render the row with caching and clamping (now includes status if computed)
            if let Some(pb) = progress_bars.get(item_idx) {
                let rendered = layout.format_list_item_line(info, current_worktree_path.as_ref());
                let clamped = clamp(&rendered);

                // Only update if content changed
                if clamped != last_lines[item_idx] {
                    last_lines[item_idx] = clamped.clone();
                    pb.set_message(clamped);
                }
            }
        }
    });

    // Finalize progress bars: no clearing race; footer morphs into summary on TTY
    if show_progress {
        use std::io::IsTerminal;
        let is_tty = std::io::stderr().is_terminal(); // Check stderr, not stdout ✅

        // Build final summary string once using helper function
        let final_msg =
            super::format_summary_message(&all_items, show_branches, layout.hidden_nonempty_count);

        if is_tty {
            // Interactive: morph footer → summary, keep rows in place
            if let Some(pb) = spacer_pb.as_ref() {
                pb.finish(); // leave the blank line
            }
            if let Some(pb) = footer_pb.as_ref() {
                pb.finish_with_message(final_msg.clone());
            }
            if let Some(pb) = header_pb {
                pb.finish();
            }
            for pb in progress_bars {
                pb.finish();
            }
        } else {
            // Non-TTY: clear progress bars and print final table to stderr
            if let Some(pb) = spacer_pb {
                pb.finish_and_clear();
            }
            if let Some(pb) = footer_pb {
                pb.finish_and_clear();
            }
            if let Some(pb) = header_pb {
                pb.finish_and_clear();
            }
            for pb in progress_bars {
                pb.finish_and_clear();
            }
            // Ensure atomicity w.r.t. indicatif's draw thread
            multi.suspend(|| {
                // Redraw static table
                crate::output::raw_terminal(layout.format_header_line())?;
                for item in &all_items {
                    crate::output::raw_terminal(
                        layout.format_list_item_line(item, current_worktree_path.as_ref()),
                    )?;
                }
                // Blank line + summary (rendered here in non-tty mode)
                crate::output::raw_terminal("")?;
                crate::output::raw_terminal(final_msg.clone())
            })?;
        }
    } else {
        for pb in progress_bars {
            pb.finish();
        }
    }

    // Compute status_symbols for any items that didn't get computed during progressive loading
    // (fallback for buffered mode or if progressive computation was skipped for some reason)
    compute_all_status_symbols(&mut all_items, &sorted_worktrees, &primary);

    // Buffered mode: compute display fields (progressive mode doesn't need this as rendering uses raw data)
    if !show_progress {
        for info in &mut all_items {
            info.display = super::model::DisplayFields::from_common_fields(
                &info.counts,
                &info.branch_diff,
                &info.upstream,
                &info.pr_status,
            );

            if let super::model::ItemKind::Worktree(ref mut wt_data) = info.kind
                && let Some(ref working_tree_diff) = wt_data.working_tree_diff
            {
                wt_data.working_diff_display = super::columns::ColumnKind::WorkingDiff
                    .format_diff_plain(working_tree_diff.added, working_tree_diff.deleted);
            }
        }
    }

    // all_items now contains both worktrees and branches (if requested)
    let items = all_items;

    // Progressive mode: table already rendered via progress bars (finished in place for TTY,
    // or explicitly printed to stderr for non-TTY in the finalization code above)
    // Buffered mode: table will be rendered in mod.rs
    // Both modes: summary will be rendered in mod.rs

    Ok(Some(super::model::ListData {
        items,
        current_worktree_path,
    }))
}

/// Sort worktrees for display (primary first, then current, then by timestamp descending).
fn sort_worktrees(
    worktrees: &[Worktree],
    primary: &Worktree,
    current_path: Option<&std::path::PathBuf>,
) -> Vec<Worktree> {
    let timestamps: Vec<i64> = worktrees
        .par_iter()
        .map(|wt| {
            Repository::at(&wt.path)
                .commit_timestamp(&wt.head)
                .unwrap_or(0)
        })
        .collect();

    let mut indexed: Vec<_> = worktrees.iter().enumerate().collect();
    indexed.sort_by_key(|(idx, wt)| {
        let is_primary = wt.path == primary.path;
        let is_current = current_path.map(|cp| &wt.path == cp).unwrap_or(false);

        let priority = if is_primary {
            0
        } else if is_current {
            1
        } else {
            2
        };

        (priority, std::cmp::Reverse(timestamps[*idx]))
    });

    indexed.into_iter().map(|(_, wt)| wt.clone()).collect()
}

mod layout;
mod render;

#[cfg(test)]
mod spacing_test;

use rayon::prelude::*;
use worktrunk::git::{GitError, Repository};
use worktrunk::styling::{HINT, HINT_EMOJI, WARNING, WARNING_EMOJI, eprintln};

use layout::calculate_responsive_layout;
use render::{format_header_line, format_list_item_line};

#[derive(serde::Serialize)]
pub struct WorktreeInfo {
    pub worktree: worktrunk::git::Worktree,
    #[serde(flatten)]
    pub commit: CommitDetails,
    #[serde(flatten)]
    pub counts: AheadBehind,
    pub working_tree_diff: (usize, usize),
    #[serde(flatten)]
    pub branch_diff: BranchDiffTotals,
    pub is_primary: bool,
    #[serde(flatten)]
    pub upstream: UpstreamStatus,
    pub worktree_state: Option<String>,
}

#[derive(serde::Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub head: String,
    #[serde(flatten)]
    pub commit: CommitDetails,
    #[serde(flatten)]
    pub counts: AheadBehind,
    #[serde(flatten)]
    pub branch_diff: BranchDiffTotals,
    #[serde(flatten)]
    pub upstream: UpstreamStatus,
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct CommitDetails {
    pub timestamp: i64,
    pub commit_message: String,
}

impl CommitDetails {
    fn gather(repo: &Repository, head: &str) -> Result<Self, GitError> {
        Ok(Self {
            timestamp: repo.commit_timestamp(head)?,
            commit_message: repo.commit_message(head)?,
        })
    }
}

#[derive(serde::Serialize, Default, Clone)]
pub(crate) struct AheadBehind {
    pub ahead: usize,
    pub behind: usize,
}

impl AheadBehind {
    fn compute(repo: &Repository, base: Option<&str>, head: &str) -> Result<Self, GitError> {
        let Some(base) = base else {
            return Ok(Self::default());
        };

        let (ahead, behind) = repo.ahead_behind(base, head)?;
        Ok(Self { ahead, behind })
    }
}

#[derive(serde::Serialize, Default, Clone)]
pub(crate) struct BranchDiffTotals {
    #[serde(rename = "branch_diff")]
    pub diff: (usize, usize),
}

impl BranchDiffTotals {
    fn compute(repo: &Repository, base: Option<&str>, head: &str) -> Result<Self, GitError> {
        let Some(base) = base else {
            return Ok(Self::default());
        };

        let diff = repo.branch_diff_stats(base, head)?;
        Ok(Self { diff })
    }
}

#[derive(serde::Serialize, Default, Clone)]
pub(crate) struct UpstreamStatus {
    #[serde(rename = "upstream_remote")]
    remote: Option<String>,
    #[serde(rename = "upstream_ahead")]
    ahead: usize,
    #[serde(rename = "upstream_behind")]
    behind: usize,
}

impl UpstreamStatus {
    fn calculate(repo: &Repository, branch: Option<&str>, head: &str) -> Result<Self, GitError> {
        let Some(branch) = branch else {
            return Ok(Self::default());
        };

        match repo.upstream_branch(branch) {
            Ok(Some(upstream_branch)) => {
                let remote = upstream_branch
                    .split_once('/')
                    .map(|(remote, _)| remote)
                    .unwrap_or("origin")
                    .to_string();
                let (ahead, behind) = repo.ahead_behind(&upstream_branch, head)?;
                Ok(Self {
                    remote: Some(remote),
                    ahead,
                    behind,
                })
            }
            _ => Ok(Self::default()),
        }
    }

    fn active(&self) -> Option<(&str, usize, usize)> {
        if self.ahead == 0 && self.behind == 0 {
            None
        } else {
            Some((
                self.remote.as_deref().unwrap_or("origin"),
                self.ahead,
                self.behind,
            ))
        }
    }
}

/// Unified type for displaying worktrees and branches in the same table
#[derive(serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ListItem {
    Worktree(WorktreeInfo),
    Branch(BranchInfo),
}

impl ListItem {
    pub fn branch_name(&self) -> &str {
        match self {
            ListItem::Worktree(wt) => wt.worktree.branch.as_deref().unwrap_or("(detached)"),
            ListItem::Branch(br) => &br.name,
        }
    }

    pub fn is_primary(&self) -> bool {
        match self {
            ListItem::Worktree(wt) => wt.is_primary,
            ListItem::Branch(_) => false,
        }
    }

    pub fn commit_timestamp(&self) -> i64 {
        match self {
            ListItem::Worktree(info) => info.commit.timestamp,
            ListItem::Branch(info) => info.commit.timestamp,
        }
    }

    pub fn head(&self) -> &str {
        match self {
            ListItem::Worktree(info) => &info.worktree.head,
            ListItem::Branch(info) => &info.head,
        }
    }

    pub fn commit_details(&self) -> &CommitDetails {
        match self {
            ListItem::Worktree(info) => &info.commit,
            ListItem::Branch(info) => &info.commit,
        }
    }

    pub fn counts(&self) -> &AheadBehind {
        match self {
            ListItem::Worktree(info) => &info.counts,
            ListItem::Branch(info) => &info.counts,
        }
    }

    pub fn branch_diff(&self) -> &BranchDiffTotals {
        match self {
            ListItem::Worktree(info) => &info.branch_diff,
            ListItem::Branch(info) => &info.branch_diff,
        }
    }

    pub fn upstream(&self) -> &UpstreamStatus {
        match self {
            ListItem::Worktree(info) => &info.upstream,
            ListItem::Branch(info) => &info.upstream,
        }
    }

    pub fn worktree_info(&self) -> Option<&WorktreeInfo> {
        match self {
            ListItem::Worktree(info) => Some(info),
            ListItem::Branch(_) => None,
        }
    }

    pub fn worktree_path(&self) -> Option<&std::path::PathBuf> {
        self.worktree_info().map(|info| &info.worktree.path)
    }
}

impl BranchInfo {
    /// Create BranchInfo from a branch name, enriching it with git metadata
    fn from_branch(
        branch: &str,
        repo: &Repository,
        primary_branch: Option<&str>,
    ) -> Result<Self, GitError> {
        // Get the commit SHA for this branch
        let head = repo.run_command(&["rev-parse", branch])?.trim().to_string();

        let commit = CommitDetails::gather(repo, &head)?;
        let counts = AheadBehind::compute(repo, primary_branch, &head)?;
        let branch_diff = BranchDiffTotals::compute(repo, primary_branch, &head)?;
        let upstream = UpstreamStatus::calculate(repo, Some(branch), &head)?;

        Ok(BranchInfo {
            name: branch.to_string(),
            head,
            commit,
            counts,
            branch_diff,
            upstream,
        })
    }
}

impl WorktreeInfo {
    /// Create WorktreeInfo from a Worktree, enriching it with git metadata
    fn from_worktree(
        wt: &worktrunk::git::Worktree,
        primary: &worktrunk::git::Worktree,
    ) -> Result<Self, GitError> {
        let wt_repo = Repository::at(&wt.path);
        let is_primary = wt.path == primary.path;

        let commit = CommitDetails::gather(&wt_repo, &wt.head)?;
        let base_branch = primary.branch.as_deref().filter(|_| !is_primary);
        let counts = AheadBehind::compute(&wt_repo, base_branch, &wt.head)?;

        let working_tree_diff = wt_repo.working_tree_diff_stats()?;
        let branch_diff = BranchDiffTotals::compute(&wt_repo, base_branch, &wt.head)?;
        let upstream = UpstreamStatus::calculate(&wt_repo, wt.branch.as_deref(), &wt.head)?;

        // Get worktree state (merge/rebase/etc)
        let worktree_state = wt_repo.worktree_state()?;

        Ok(WorktreeInfo {
            worktree: wt.clone(),
            commit,
            counts,
            working_tree_diff,
            branch_diff,
            is_primary,
            upstream,
            worktree_state,
        })
    }
}

pub fn handle_list(format: crate::OutputFormat, show_branches: bool) -> Result<(), GitError> {
    let repo = Repository::current();
    let worktrees = repo.list_worktrees()?;

    if worktrees.is_empty() {
        return Ok(());
    }

    // First worktree is the primary - clone it for use in closure
    let primary = worktrees[0].clone();

    // Get current worktree to identify active one
    let current_worktree_path = repo.worktree_root().ok();

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
    let worktree_infos: Vec<WorktreeInfo> = worktrees
        .par_iter()
        .map(|wt| WorktreeInfo::from_worktree(wt, &primary))
        .collect::<Result<Vec<_>, _>>()?;

    // Build list of items to display (worktrees + optional branches)
    let mut items: Vec<ListItem> = worktree_infos.into_iter().map(ListItem::Worktree).collect();

    // Add branches if requested
    if show_branches {
        let available_branches = repo.available_branches()?;
        let primary_branch = primary.branch.as_deref();
        for branch in available_branches {
            match BranchInfo::from_branch(&branch, &repo, primary_branch) {
                Ok(branch_info) => items.push(ListItem::Branch(branch_info)),
                Err(e) => {
                    let warning_bold = WARNING.bold();
                    eprintln!(
                        "{WARNING_EMOJI} {WARNING}Failed to enrich branch {warning_bold}{branch}{warning_bold:#}: {e}{WARNING:#}"
                    );
                    eprintln!(
                        "{HINT_EMOJI} {HINT}This branch will be shown with limited information{HINT:#}"
                    );
                }
            }
        }
    }

    // Sort by most recent commit (descending)
    items.sort_by_key(|item| std::cmp::Reverse(item.commit_timestamp()));

    match format {
        crate::OutputFormat::Json => {
            // Output JSON format
            let json = serde_json::to_string_pretty(&items).map_err(|e| {
                GitError::CommandFailed(format!("Failed to serialize to JSON: {}", e))
            })?;
            println!("{}", json);
        }
        crate::OutputFormat::Table => {
            // Calculate responsive layout based on terminal width
            let layout = calculate_responsive_layout(&items);

            // Display header
            format_header_line(&layout);

            // Display formatted output
            for item in &items {
                format_list_item_line(item, &layout, current_worktree_path.as_ref());
            }

            // Display summary line
            display_summary(&items, show_branches);
        }
    }

    Ok(())
}

fn display_summary(items: &[ListItem], include_branches: bool) {
    use anstyle::Style;
    use worktrunk::styling::println;

    if items.is_empty() {
        println!();
        use worktrunk::styling::{HINT, HINT_EMOJI};
        println!("{HINT_EMOJI} {HINT}No worktrees found{HINT:#}");
        println!("{HINT_EMOJI} {HINT}Create one with: wt switch --create <branch>{HINT:#}");
        return;
    }

    let mut metrics = SummaryMetrics::default();
    for item in items {
        metrics.update(item);
    }

    println!();
    let dim = Style::new().dimmed();

    // Build summary parts
    let mut parts = Vec::new();

    if include_branches {
        parts.push(format!("{} worktrees", metrics.worktrees));
        if metrics.branches > 0 {
            parts.push(format!("{} branches", metrics.branches));
        }
    } else {
        let plural = if metrics.worktrees == 1 { "" } else { "s" };
        parts.push(format!("{} worktree{}", metrics.worktrees, plural));
    }

    if metrics.dirty_worktrees > 0 {
        parts.push(format!("{} with changes", metrics.dirty_worktrees));
    }

    if metrics.ahead_items > 0 {
        parts.push(format!("{} ahead", metrics.ahead_items));
    }

    if metrics.behind_items > 0 {
        parts.push(format!("{} behind", metrics.behind_items));
    }

    let summary = parts.join(", ");
    println!("{dim}Showing {summary}{dim:#}");
}

#[derive(Default)]
struct SummaryMetrics {
    worktrees: usize,
    branches: usize,
    dirty_worktrees: usize,
    ahead_items: usize,
    behind_items: usize,
}

impl SummaryMetrics {
    fn update(&mut self, item: &ListItem) {
        if let Some(info) = item.worktree_info() {
            self.worktrees += 1;
            let (added, deleted) = info.working_tree_diff;
            if added > 0 || deleted > 0 {
                self.dirty_worktrees += 1;
            }
        } else {
            self.branches += 1;
        }

        let counts = item.counts();
        if counts.ahead > 0 {
            self.ahead_items += 1;
        }
        if counts.behind > 0 {
            self.behind_items += 1;
        }
    }
}

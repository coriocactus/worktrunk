//! Git operations and repository management

use std::path::PathBuf;

// Submodules
mod error;
mod parse;
mod repository;

#[cfg(test)]
mod test;

// Re-exports from submodules
pub use error::GitError;
pub use repository::{GitResultExt, Repository};

// Re-export parsing functions for internal use
pub(crate) use parse::{
    parse_local_default_branch, parse_numstat, parse_remote_default_branch, parse_worktree_list,
};

// Note: HookType and Worktree are defined in this module and are already public.
// They're accessible as git::HookType and git::Worktree without needing re-export.

/// Hook types for git operations
#[derive(Debug, Clone, Copy, clap::ValueEnum, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum HookType {
    PostCreate,
    PostStart,
    PreCommit,
    PreMerge,
    PostMerge,
}

/// Worktree information
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

// Helper functions for worktree parsing
//
// These live in mod.rs rather than parse.rs because they bridge multiple concerns:
// - read_rebase_branch() uses Repository (from repository.rs) to access git internals
// - finalize_worktree() operates on Worktree (defined here in mod.rs)
// - Both are tightly coupled to the Worktree type definition
//
// Placing them here avoids circular dependencies and keeps them close to Worktree.

/// Helper function to read rebase branch information
fn read_rebase_branch(worktree_path: &PathBuf) -> Option<String> {
    // Create a Repository instance to get the correct git directory
    let repo = Repository::at(worktree_path);
    let git_dir = repo.git_dir().ok()?;

    // Check both rebase-merge and rebase-apply
    for rebase_dir in ["rebase-merge", "rebase-apply"] {
        let head_name_path = git_dir.join(rebase_dir).join("head-name");
        if let Ok(content) = std::fs::read_to_string(head_name_path) {
            let branch_ref = content.trim();
            // Strip refs/heads/ prefix if present
            let branch = branch_ref
                .strip_prefix("refs/heads/")
                .unwrap_or(branch_ref)
                .to_string();
            return Some(branch);
        }
    }

    None
}

/// Finalize a worktree after parsing, filling in branch name from rebase state if needed.
pub(crate) fn finalize_worktree(mut wt: Worktree) -> Worktree {
    // If detached but no branch, check if we're rebasing
    if wt.detached
        && wt.branch.is_none()
        && let Some(branch) = read_rebase_branch(&wt.path)
    {
        wt.branch = Some(branch);
    }
    wt
}

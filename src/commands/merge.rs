use std::path::Path;
use worktrunk::config::load_config;
use worktrunk::error_format::format_error;
use worktrunk::git::{
    GitError, count_commits_in, get_commit_subjects_in, get_current_branch_in,
    get_default_branch_in, get_merge_base_in, get_repo_root_in, has_staged_changes_in, is_dirty_in,
    run_git_command,
};

use super::worktree::handle_push;
use super::worktree::handle_remove;

pub fn handle_merge(target: Option<&str>, squash: bool, keep: bool) -> Result<(), GitError> {
    // Get current branch
    let current_branch = get_current_branch_in(Path::new("."))?
        .ok_or_else(|| GitError::CommandFailed(format_error("Not on a branch (detached HEAD)")))?;

    // Get target branch (default to default branch if not provided)
    let target_branch = match target {
        Some(b) => b.to_string(),
        None => get_default_branch_in(Path::new("."))?,
    };

    // Check if already on target branch
    if current_branch == target_branch {
        println!("Already on '{}', nothing to merge", target_branch);
        return Ok(());
    }

    // Check for uncommitted changes
    if is_dirty_in(Path::new("."))? {
        return Err(GitError::CommandFailed(format_error(
            "Working tree has uncommitted changes. Commit or stash them first.",
        )));
    }

    // Squash commits if requested
    if squash {
        handle_squash(&target_branch)?;
    }

    // Rebase onto target
    println!("Rebasing onto '{}'...", target_branch);

    run_git_command(&["rebase", &target_branch], Some(Path::new("."))).map_err(|e| {
        GitError::CommandFailed(format!("Failed to rebase onto '{}': {}", target_branch, e))
    })?;

    // Fast-forward push to target branch (reuse handle_push logic)
    println!("Fast-forwarding '{}' to current HEAD...", target_branch);
    handle_push(Some(&target_branch), false)?;

    // Finish worktree unless --keep was specified
    if !keep {
        println!("Cleaning up worktree...");

        // Get primary worktree path before finishing (while we can still run git commands)
        let primary_worktree_dir = get_repo_root_in(Path::new("."))?;

        handle_remove(false)?;

        // Check if we need to switch to target branch
        let new_branch = get_current_branch_in(&primary_worktree_dir)?;
        if new_branch.as_deref() != Some(&target_branch) {
            println!("Switching to '{}'...", target_branch);
            run_git_command(&["switch", &target_branch], Some(&primary_worktree_dir)).map_err(
                |e| {
                    GitError::CommandFailed(format!(
                        "Failed to switch to '{}': {}",
                        target_branch, e
                    ))
                },
            )?;
        }
    } else {
        println!(
            "Successfully merged to '{}' (worktree preserved)",
            target_branch
        );
    }

    Ok(())
}

fn handle_squash(target_branch: &str) -> Result<(), GitError> {
    // Get merge base with target branch
    let merge_base = get_merge_base_in(Path::new("."), "HEAD", target_branch)?;

    // Count commits since merge base
    let commit_count = count_commits_in(Path::new("."), &merge_base, "HEAD")?;

    // Check if there are staged changes
    let has_staged = has_staged_changes_in(Path::new("."))?;

    // Handle different scenarios
    if commit_count == 0 && !has_staged {
        // No commits and no staged changes - nothing to squash
        println!("No commits to squash - already at merge base");
        return Ok(());
    }

    if commit_count == 0 && has_staged {
        // Just staged changes, no commits - would need to commit but this shouldn't happen in merge flow
        return Err(GitError::CommandFailed(format_error(
            "Staged changes without commits - please commit them first",
        )));
    }

    if commit_count == 1 && !has_staged {
        // Single commit, no staged changes - nothing to do
        println!(
            "Only 1 commit since '{}' - no squashing needed",
            target_branch
        );
        return Ok(());
    }

    // One or more commits (possibly with staged changes) - squash them
    println!("Squashing {} commits into one...", commit_count);

    // Get commit subjects for the squash message
    let range = format!("{}..HEAD", merge_base);
    let subjects = get_commit_subjects_in(Path::new("."), &range)?;

    // Load config and generate commit message
    let config = load_config()
        .map_err(|e| GitError::CommandFailed(format!("Failed to load config: {}", e)))?;
    let commit_message = crate::llm::generate_squash_message(target_branch, &subjects, &config.llm);

    // Reset to merge base (soft reset stages all changes)
    run_git_command(&["reset", "--soft", &merge_base], Some(Path::new(".")))
        .map_err(|e| GitError::CommandFailed(format!("Failed to reset to merge base: {}", e)))?;

    // Commit with the generated message
    run_git_command(&["commit", "-m", &commit_message], Some(Path::new(".")))
        .map_err(|e| GitError::CommandFailed(format!("Failed to create squash commit: {}", e)))?;

    println!("Successfully squashed {} commits into one", commit_count);
    Ok(())
}

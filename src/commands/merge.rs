use worktrunk::config::{ProjectConfig, WorktrunkConfig, expand_command_template};
use worktrunk::git::{GitError, Repository};
use worktrunk::styling::{
    AnstyleStyle, CYAN, CYAN_BOLD, ERROR, ERROR_EMOJI, GREEN, GREEN_BOLD, HINT, HINT_EMOJI,
    eprintln, println,
};

use super::command_approval::{approve_command_batch, command_config_to_vec};
use super::worktree::handle_push;
use super::worktree::handle_remove;
use crate::output::{execute_command_in_worktree, handle_remove_output};

pub fn handle_merge(
    target: Option<&str>,
    squash: bool,
    keep: bool,
    message: Option<&str>,
    no_verify: bool,
    force: bool,
    internal: bool,
) -> Result<(), GitError> {
    let repo = Repository::current();

    // Get current branch
    let current_branch = repo.current_branch()?.ok_or_else(|| {
        eprintln!("{ERROR_EMOJI} {ERROR}Not on a branch (detached HEAD){ERROR:#}");
        eprintln!();
        eprintln!("{HINT_EMOJI} {HINT}You are in detached HEAD state{HINT:#}");
        GitError::CommandFailed(String::new())
    })?;

    // Get target branch (default to default branch if not provided)
    let target_branch = target.map_or_else(|| repo.default_branch(), |b| Ok(b.to_string()))?;

    // Check if already on target branch
    if current_branch == target_branch {
        println!(
            "✅ {GREEN}Already on {GREEN_BOLD}{target_branch}{GREEN_BOLD:#}, nothing to merge{GREEN:#}"
        );
        return Ok(());
    }

    // Load config for LLM integration
    let config = WorktrunkConfig::load()
        .map_err(|e| GitError::CommandFailed(format!("Failed to load config: {}", e)))?;

    // Auto-commit uncommitted changes if they exist
    if repo.is_dirty()? {
        handle_commit_changes(message, &config.commit_generation)?;
    }

    // Run pre-merge checks unless --no-verify was specified
    if !no_verify && let Ok(Some(project_config)) = ProjectConfig::load(&repo.worktree_root()?) {
        let worktree_path = std::env::current_dir().map_err(|e| {
            GitError::CommandFailed(format!("Failed to get current directory: {}", e))
        })?;
        run_pre_merge_checks(
            &project_config,
            &current_branch,
            &target_branch,
            &worktree_path,
            &repo,
            &config,
            force,
            internal,
        )?;
    }

    // Track operations for summary
    let mut squashed_count: Option<usize> = None;

    // Squash commits if requested
    if squash {
        squashed_count = handle_squash(&target_branch, internal)?;
    }

    // Rebase onto target
    if !internal {
        println!("🔄 {CYAN}Rebasing onto {CYAN_BOLD}{target_branch}{CYAN_BOLD:#}...{CYAN:#}");
    }

    repo.run_command(&["rebase", &target_branch]).map_err(|e| {
        GitError::CommandFailed(format!("Failed to rebase onto '{}': {}", target_branch, e))
    })?;

    // Fast-forward push to target branch (reuse handle_push logic)
    handle_push(Some(&target_branch), false, internal)?;

    // Finish worktree unless --keep was specified
    if !keep {
        if !internal {
            println!("🔄 {CYAN}Cleaning up worktree...{CYAN:#}");
        }

        // Get primary worktree path before finishing (while we can still run git commands)
        let primary_worktree_dir = repo.main_worktree_root()?;

        let result = handle_remove(None)?;

        // Display output based on mode
        handle_remove_output(&result)?;

        // Check if we need to switch to target branch
        let primary_repo = Repository::at(&primary_worktree_dir);
        let new_branch = primary_repo.current_branch()?;
        if new_branch.as_deref() != Some(&target_branch) {
            if !internal {
                println!(
                    "🔄 {CYAN}Switching to {CYAN_BOLD}{target_branch}{CYAN_BOLD:#}...{CYAN:#}"
                );
            }
            primary_repo
                .run_command(&["switch", &target_branch])
                .map_err(|e| {
                    GitError::CommandFailed(format!(
                        "Failed to switch to '{}': {}",
                        target_branch, e
                    ))
                })?;
        }

        // Print comprehensive summary
        println!();
        handle_merge_summary_output(&current_branch, &target_branch, squashed_count, true)?;
    } else {
        // Print comprehensive summary (worktree preserved)
        println!();
        handle_merge_summary_output(&current_branch, &target_branch, squashed_count, false)?;
    }

    Ok(())
}

/// Format the merge summary message
fn format_merge_summary(
    from_branch: &str,
    to_branch: &str,
    squashed_count: Option<usize>,
    cleaned_up: bool,
) -> String {
    let bold = AnstyleStyle::new().bold();
    let dim = AnstyleStyle::new().dimmed();

    let mut output = format!("Merge complete\n\n");

    // Show what was merged
    output.push_str(&format!(
        "  {dim}Merged: {bold}{from_branch}{bold:#} → {bold}{to_branch}{bold:#}{dim:#}\n"
    ));

    // Show squash info if applicable
    if let Some(count) = squashed_count {
        output.push_str(&format!("  {dim}Squashed: {count} commits into 1{dim:#}\n"));
    }

    // Show worktree status
    if cleaned_up {
        output.push_str(&format!("  {dim}Worktree: Removed{dim:#}"));
    } else {
        output.push_str(&format!(
            "  {dim}Worktree: Kept (use 'wt remove' to clean up){dim:#}"
        ));
    }

    output
}

/// Handle output for merge summary using global output context
fn handle_merge_summary_output(
    from_branch: &str,
    to_branch: &str,
    squashed_count: Option<usize>,
    cleaned_up: bool,
) -> Result<(), GitError> {
    let message = format_merge_summary(from_branch, to_branch, squashed_count, cleaned_up);

    // Show success message (formatting added by OutputContext)
    crate::output::success(message).map_err(|e| GitError::CommandFailed(e.to_string()))?;

    // Flush output
    crate::output::flush().map_err(|e| GitError::CommandFailed(e.to_string()))?;

    Ok(())
}

/// Commit uncommitted changes with LLM-generated message
fn handle_commit_changes(
    custom_instruction: Option<&str>,
    commit_generation_config: &worktrunk::config::CommitGenerationConfig,
) -> Result<(), GitError> {
    let repo = Repository::current();

    println!("🔄 {CYAN}Committing uncommitted changes...{CYAN:#}");

    // Stage all tracked changes (excludes untracked files)
    repo.run_command(&["add", "-u"])
        .map_err(|e| GitError::CommandFailed(format!("Failed to stage changes: {}", e)))?;

    // Check if there are staged changes after staging
    if !repo.has_staged_changes()? {
        // No staged changes means only untracked files exist
        eprintln!("{ERROR_EMOJI} {ERROR}Working tree has untracked files{ERROR:#}");
        eprintln!();
        eprintln!("{HINT_EMOJI} {HINT}Add them with 'git add' and try again{HINT:#}");
        return Err(GitError::CommandFailed(String::new()));
    }

    // Generate commit message
    let commit_message =
        crate::llm::generate_commit_message(custom_instruction, commit_generation_config)?;

    // Commit
    repo.run_command(&["commit", "-m", &commit_message])
        .map_err(|e| GitError::CommandFailed(format!("Failed to commit: {}", e)))?;

    println!("✅ {GREEN}Committed changes{GREEN:#}");

    Ok(())
}

fn handle_squash(target_branch: &str, internal: bool) -> Result<Option<usize>, GitError> {
    let repo = Repository::current();

    // Get merge base with target branch
    let merge_base = repo.merge_base("HEAD", target_branch)?;

    // Count commits since merge base
    let commit_count = repo.count_commits(&merge_base, "HEAD")?;

    // Check if there are staged changes
    let has_staged = repo.has_staged_changes()?;

    // Handle different scenarios
    if commit_count == 0 && !has_staged {
        // No commits and no staged changes - nothing to squash
        if !internal {
            let dim = AnstyleStyle::new().dimmed();
            println!("{dim}No commits to squash - already at merge base{dim:#}");
        }
        return Ok(None);
    }

    if commit_count == 0 && has_staged {
        // Just staged changes, no commits - would need to commit but this shouldn't happen in merge flow
        eprintln!("{ERROR_EMOJI} {ERROR}Staged changes without commits{ERROR:#}");
        eprintln!();
        eprintln!("{HINT_EMOJI} {HINT}Please commit them first{HINT:#}");
        return Err(GitError::CommandFailed(String::new()));
    }

    if commit_count == 1 && !has_staged {
        // Single commit, no staged changes - nothing to do
        if !internal {
            let dim = AnstyleStyle::new().dimmed();
            println!(
                "{dim}Only 1 commit since {CYAN_BOLD}{target_branch}{CYAN_BOLD:#} - no squashing needed{dim:#}"
            );
        }
        return Ok(None);
    }

    // One or more commits (possibly with staged changes) - squash them
    if !internal {
        println!("🔄 {CYAN}Squashing {commit_count} commits into one...{CYAN:#}");
    }

    // Get commit subjects for the squash message
    let range = format!("{}..HEAD", merge_base);
    let subjects = repo.commit_subjects(&range)?;

    // Load config and generate commit message
    let config = WorktrunkConfig::load()
        .map_err(|e| GitError::CommandFailed(format!("Failed to load config: {}", e)))?;
    let commit_message =
        crate::llm::generate_squash_message(target_branch, &subjects, &config.commit_generation)
            .map_err(|e| {
                GitError::CommandFailed(format!("Failed to generate commit message: {}", e))
            })?;

    // Reset to merge base (soft reset stages all changes)
    repo.run_command(&["reset", "--soft", &merge_base])
        .map_err(|e| GitError::CommandFailed(format!("Failed to reset to merge base: {}", e)))?;

    // Commit with the generated message
    repo.run_command(&["commit", "-m", &commit_message])
        .map_err(|e| GitError::CommandFailed(format!("Failed to create squash commit: {}", e)))?;

    if !internal {
        println!("✅ {GREEN}Squashed {commit_count} commits into one{GREEN:#}");
    }
    Ok(Some(commit_count))
}

/// Run pre-merge checks sequentially (blocking, fail-fast)
fn run_pre_merge_checks(
    project_config: &ProjectConfig,
    current_branch: &str,
    target_branch: &str,
    worktree_path: &std::path::Path,
    repo: &Repository,
    config: &WorktrunkConfig,
    force: bool,
    internal: bool,
) -> Result<(), GitError> {
    let Some(pre_merge_config) = &project_config.pre_merge_check else {
        return Ok(());
    };

    let commands = command_config_to_vec(pre_merge_config, "cmd");
    if commands.is_empty() {
        return Ok(());
    }

    let project_id = repo.project_identifier()?;
    let repo_root = repo.main_worktree_root()?;
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Approve commands (prompt if needed, save approvals)
    if !approve_command_batch(&commands, &project_id, config, force, "Pre-merge checks")? {
        return Err(GitError::CommandFailed(String::new()));
    }

    // Execute each command sequentially, fail-fast on errors
    for (name, command) in commands {
        let expanded_command = expand_command_template(
            &command,
            repo_name,
            current_branch,
            worktree_path,
            &repo_root,
            Some(target_branch),
        );

        if !internal {
            use std::io::Write;
            println!("🔄 {CYAN}Running pre-merge check '{name}'...{CYAN:#}");
            let _ = std::io::stdout().flush();
        }

        if let Err(e) = execute_command_in_worktree(worktree_path, &expanded_command) {
            eprintln!();
            let error_bold = ERROR.bold();
            eprintln!(
                "{ERROR_EMOJI} {ERROR}Pre-merge check failed: {error_bold}{name}{error_bold:#}{ERROR:#}"
            );
            eprintln!();
            eprintln!("  {e}");
            eprintln!();
            eprintln!("{HINT_EMOJI} {HINT}Use --no-verify to skip pre-merge checks{HINT:#}");
            return Err(GitError::CommandFailed(String::new()));
        }
    }

    Ok(())
}

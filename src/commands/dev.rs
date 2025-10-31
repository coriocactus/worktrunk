use worktrunk::config::{ProjectConfig, WorktrunkConfig};
use worktrunk::git::{GitError, GitResultExt, Repository};
use worktrunk::styling::{HINT, HINT_EMOJI};

use super::merge::{
    execute_post_merge_commands, run_pre_commit_commands, run_pre_merge_commands,
    run_pre_squash_commands,
};
use super::worktree::{execute_post_create_commands, execute_post_start_commands_sequential};

// Re-export HookType from main
pub use crate::HookType;

/// Handle `wt dev run-hook` command
pub fn handle_dev_run_hook(hook_type: HookType, force: bool) -> Result<(), GitError> {
    // Derive context from current environment
    let repo = Repository::current();
    let worktree_path = std::env::current_dir()
        .map_err(|e| GitError::CommandFailed(format!("Failed to get current directory: {}", e)))?;
    let branch = repo
        .current_branch()
        .git_context("Failed to get current branch")?
        .ok_or_else(|| GitError::CommandFailed("Not on a branch (detached HEAD)".to_string()))?;
    let config = WorktrunkConfig::load().git_context("Failed to load config")?;

    // Load project config (show helpful error if missing)
    let project_config = load_project_config(&repo)?;

    // TODO: Add support for custom variable overrides (e.g., --var key=value)
    // This would allow testing hooks with different contexts without being in that context

    // Execute the hook based on type
    match hook_type {
        HookType::PostCreate => {
            check_hook_configured(&project_config.post_create_command, hook_type)?;
            execute_post_create_commands(&worktree_path, &repo, &config, &branch, force)
        }
        HookType::PostStart => {
            check_hook_configured(&project_config.post_start_command, hook_type)?;
            execute_post_start_commands_sequential(&worktree_path, &repo, &config, &branch, force)
        }
        HookType::PreCommit => {
            check_hook_configured(&project_config.pre_commit_command, hook_type)?;
            run_pre_commit_commands(
                &project_config,
                &branch,
                &worktree_path,
                &repo,
                &config,
                force,
            )
        }
        HookType::PreSquash => {
            check_hook_configured(&project_config.pre_squash_command, hook_type)?;
            let target_branch = repo.default_branch().unwrap_or_else(|_| "main".to_string());
            run_pre_squash_commands(
                &project_config,
                &branch,
                &target_branch,
                &worktree_path,
                &repo,
                &config,
                force,
            )
        }
        HookType::PreMerge => {
            check_hook_configured(&project_config.pre_merge_command, hook_type)?;
            let target_branch = repo.default_branch().unwrap_or_else(|_| "main".to_string());
            run_pre_merge_commands(
                &project_config,
                &branch,
                &target_branch,
                &worktree_path,
                &repo,
                &config,
                force,
            )
        }
        HookType::PostMerge => {
            check_hook_configured(&project_config.post_merge_command, hook_type)?;
            let target_branch = repo.default_branch().unwrap_or_else(|_| "main".to_string());
            execute_post_merge_commands(
                &worktree_path,
                &repo,
                &config,
                &branch,
                &target_branch,
                force,
            )
        }
    }
}

fn load_project_config(repo: &Repository) -> Result<ProjectConfig, GitError> {
    let repo_root = repo.worktree_root()?;
    let config_path = repo_root.join(".config").join("wt.toml");

    match ProjectConfig::load(&repo_root).git_context("Failed to load project config")? {
        Some(cfg) => Ok(cfg),
        None => {
            // No project config found - show helpful error
            use worktrunk::styling::ERROR;
            use worktrunk::styling::ERROR_EMOJI;
            let hint_bold = HINT.bold();
            eprintln!("{ERROR_EMOJI} {ERROR}No project configuration found{ERROR:#}",);
            eprintln!(
                "{HINT_EMOJI} {HINT}Create a config file at: {hint_bold}{}{hint_bold:#}{HINT:#}",
                config_path.display()
            );
            Err(GitError::CommandFailed(
                "No project configuration found".to_string(),
            ))
        }
    }
}

fn check_hook_configured<T>(hook: &Option<T>, hook_type: HookType) -> Result<(), GitError> {
    if hook.is_none() {
        let hook_name = hook_type.as_str();
        eprintln!(
            "{HINT_EMOJI} {HINT}No {hook_name} commands configured in project config{HINT:#}"
        );
        return Err(GitError::CommandFailed(format!(
            "No {hook_name} commands configured"
        )));
    }
    Ok(())
}

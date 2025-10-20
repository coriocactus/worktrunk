use std::path::Path;
use std::process;
use worktrunk::config::WorktrunkConfig;
use worktrunk::error_format::{format_error, format_error_with_bold, format_hint};
use worktrunk::git::{GitError, Repository, run_git_command};

pub fn handle_switch(
    branch: &str,
    create: bool,
    base: Option<&str>,
    internal: bool,
    config: &WorktrunkConfig,
) -> Result<(), GitError> {
    let repo = Repository::current();

    // Check for conflicting conditions
    if create && repo.branch_exists(branch)? {
        return Err(GitError::CommandFailed(format_error_with_bold(
            "Branch '",
            branch,
            "' already exists. Remove --create flag to switch to it.",
        )));
    }

    // Check if base flag was provided without create flag
    if base.is_some() && !create {
        eprintln!(
            "{}",
            format_warning("--base flag is only used with --create, ignoring")
        );
    }

    // Check if worktree already exists for this branch
    if let Some(existing_path) = repo.worktree_for_branch(branch)? {
        if existing_path.exists() {
            if internal {
                println!("__WORKTRUNK_CD__{}", existing_path.display());
            }
            return Ok(());
        } else {
            return Err(GitError::CommandFailed(format_error_with_bold(
                "Worktree directory missing for '",
                branch,
                "'. Run 'git worktree prune' to clean up.",
            )));
        }
    }

    // No existing worktree, create one
    let repo_root = repo.repo_root()?;

    let repo_name = repo_root
        .file_name()
        .ok_or_else(|| GitError::CommandFailed("Invalid repository path".to_string()))?
        .to_str()
        .ok_or_else(|| GitError::CommandFailed("Invalid UTF-8 in path".to_string()))?;

    let worktree_name = config.format_path(repo_name, branch);
    let worktree_path = repo_root.join(worktree_name);

    // Create the worktree
    // Build git worktree add command
    let mut args = vec!["worktree", "add", worktree_path.to_str().unwrap()];
    if create {
        args.push("-b");
        args.push(branch);
        if let Some(base_branch) = base {
            args.push(base_branch);
        }
    } else {
        args.push(branch);
    }

    run_git_command(&args, Some(Path::new(".")))
        .map_err(|e| GitError::CommandFailed(format!("Failed to create worktree: {}", e)))
        .map(|_| ())?;

    // Output success message
    let success_msg = if create {
        format!("Created new branch and worktree for '{}'", branch)
    } else {
        format!("Added worktree for existing branch '{}'", branch)
    };

    if internal {
        println!("__WORKTRUNK_CD__{}", worktree_path.display());
        println!("{} at {}", success_msg, worktree_path.display());
    } else {
        println!("{}", success_msg);
        print_worktree_info(&worktree_path, "switch");
    }

    Ok(())
}

pub fn handle_remove(internal: bool) -> Result<(), GitError> {
    let repo = Repository::current();

    // Check for uncommitted changes
    if repo.is_dirty()? {
        return Err(GitError::CommandFailed(format_error(
            "Working tree has uncommitted changes. Commit or stash them first.",
        )));
    }

    // Get current state
    let current_branch = repo.current_branch()?;
    let default_branch = repo.default_branch()?;
    let in_worktree = repo.is_in_worktree()?;

    // If we're on default branch and not in a worktree, nothing to do
    if !in_worktree && current_branch.as_deref() == Some(&default_branch) {
        if !internal {
            println!("Already on default branch '{}'", default_branch);
        }
        return Ok(());
    }

    if in_worktree {
        // In worktree: navigate to primary worktree and remove this one
        let worktree_root = repo.worktree_root()?;
        let primary_worktree_dir = repo.repo_root()?;

        if internal {
            println!("__WORKTRUNK_CD__{}", primary_worktree_dir.display());
        }

        // Schedule worktree removal (synchronous for now, could be async later)
        let remove_result = process::Command::new("git")
            .args(["worktree", "remove", worktree_root.to_str().unwrap()])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !remove_result.status.success() {
            let stderr = String::from_utf8_lossy(&remove_result.stderr);
            eprintln!("Warning: Failed to remove worktree: {}", stderr);
            eprintln!(
                "You may need to run 'git worktree remove {}' manually",
                worktree_root.display()
            );
        }

        if !internal {
            println!("Moved to primary worktree and removed worktree");
            print_worktree_info(&primary_worktree_dir, "remove");
        }
    } else {
        // In main repo but not on default branch: switch to default
        run_git_command(&["switch", &default_branch], Some(Path::new(".")))
            .map_err(|e| {
                GitError::CommandFailed(format!("Failed to switch to '{}': {}", default_branch, e))
            })
            .map(|_| ())?;

        if !internal {
            println!("Switched to default branch '{}'", default_branch);
        }
    }

    Ok(())
}

pub fn handle_push(target: Option<&str>, allow_merge_commits: bool) -> Result<(), GitError> {
    let repo = Repository::current();

    // Get target branch (default to default branch if not provided)
    let target_branch = match target {
        Some(b) => b.to_string(),
        None => repo.default_branch()?,
    };

    // Check if it's a fast-forward
    if !repo.is_ancestor(&target_branch, "HEAD")? {
        let error_msg =
            format_error_with_bold("Not a fast-forward from '", &target_branch, "' to HEAD");
        let hint_msg = format_hint(
            "The target branch has commits not in your current branch. Consider 'git pull' or 'git rebase'",
        );
        return Err(GitError::CommandFailed(format!(
            "{}\n{}",
            error_msg, hint_msg
        )));
    }

    // Check for merge commits unless allowed
    if !allow_merge_commits && repo.has_merge_commits(&target_branch, "HEAD")? {
        return Err(GitError::CommandFailed(format_error(
            "Found merge commits in push range. Use --allow-merge-commits to push non-linear history.",
        )));
    }

    // Configure receive.denyCurrentBranch if needed
    // TODO: These git config commands don't use run_git_command() because they don't check
    // status.success() and may rely on exit codes for missing keys. Should be refactored.
    let deny_config_output = process::Command::new("git")
        .args(["config", "receive.denyCurrentBranch"])
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    let current_config = String::from_utf8_lossy(&deny_config_output.stdout);
    if current_config.trim() != "updateInstead" {
        process::Command::new("git")
            .args(["config", "receive.denyCurrentBranch", "updateInstead"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;
    }

    // Find worktree for target branch
    let target_worktree = repo.worktree_for_branch(&target_branch)?;

    if let Some(ref wt_path) = target_worktree {
        // Check if target worktree is dirty
        if Repository::at(wt_path).is_dirty()? {
            // Get files changed in the push
            let push_files = repo.changed_files(&target_branch, "HEAD")?;

            // Get files changed in the worktree
            let wt_status_output = run_git_command(&["status", "--porcelain"], Some(wt_path))?;

            let wt_files: Vec<String> = wt_status_output
                .lines()
                .filter_map(|line| {
                    // Parse porcelain format: "XY filename"
                    line.split_once(' ')
                        .map(|(_, filename)| filename.trim().to_string())
                })
                .collect();

            // Find overlapping files
            let overlapping: Vec<String> = push_files
                .iter()
                .filter(|f| wt_files.contains(f))
                .cloned()
                .collect();

            if !overlapping.is_empty() {
                eprintln!(
                    "{}",
                    format_error("Cannot push: conflicting uncommitted changes in:")
                );
                for file in &overlapping {
                    eprintln!("  - {}", file);
                }
                return Err(GitError::CommandFailed(format!(
                    "Commit or stash changes in {} first",
                    wt_path.display()
                )));
            }
        }
    }

    // Count commits and show info
    let commit_count = repo.count_commits(&target_branch, "HEAD")?;
    if commit_count > 0 {
        let commit_text = if commit_count == 1 {
            "commit"
        } else {
            "commits"
        };
        println!(
            "Pushing {} {} to '{}'",
            commit_count, commit_text, target_branch
        );
    }

    // Get git common dir for the push
    let git_common_dir = repo.git_common_dir()?;

    // Perform the push
    let push_target = format!("HEAD:{}", target_branch);
    run_git_command(
        &["push", git_common_dir.to_str().unwrap(), &push_target],
        Some(Path::new(".")),
    )
    .map_err(|e| GitError::CommandFailed(format!("Push failed: {}", e)))?;

    println!("Successfully pushed to '{}'", target_branch);
    Ok(())
}

fn print_worktree_info(path: &std::path::Path, command: &str) {
    println!("Path: {}", path.display());
    println!(
        "Note: Use 'wt {}' (with shell integration) for automatic cd",
        command
    );
}

fn format_warning(msg: &str) -> String {
    worktrunk::error_format::format_warning(msg)
}

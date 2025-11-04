//! Git output parsing functions

use std::path::PathBuf;

use super::{GitError, Worktree, finalize_worktree};

pub(crate) fn parse_worktree_list(output: &str) -> Result<Vec<Worktree>, GitError> {
    let mut worktrees = Vec::new();
    let mut current: Option<Worktree> = None;

    for line in output.lines() {
        if line.is_empty() {
            if let Some(wt) = current.take() {
                worktrees.push(finalize_worktree(wt));
            }
            continue;
        }

        let (key, value) = match line.split_once(' ') {
            Some((k, v)) => (k, Some(v)),
            None => (line, None),
        };

        match key {
            "worktree" => {
                let path = value.ok_or_else(|| {
                    GitError::ParseError("worktree line missing path".to_string())
                })?;
                current = Some(Worktree {
                    path: PathBuf::from(path),
                    head: String::new(),
                    branch: None,
                    bare: false,
                    detached: false,
                    locked: None,
                    prunable: None,
                });
            }
            "HEAD" => {
                if let Some(ref mut wt) = current {
                    wt.head = value
                        .ok_or_else(|| GitError::ParseError("HEAD line missing SHA".to_string()))?
                        .to_string();
                }
            }
            "branch" => {
                if let Some(ref mut wt) = current {
                    // Strip refs/heads/ prefix if present
                    let branch_ref = value.ok_or_else(|| {
                        GitError::ParseError("branch line missing ref".to_string())
                    })?;
                    let branch = branch_ref
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch_ref)
                        .to_string();
                    wt.branch = Some(branch);
                }
            }
            "bare" => {
                if let Some(ref mut wt) = current {
                    wt.bare = true;
                }
            }
            "detached" => {
                if let Some(ref mut wt) = current {
                    wt.detached = true;
                }
            }
            "locked" => {
                if let Some(ref mut wt) = current {
                    wt.locked = Some(value.unwrap_or_default().to_string());
                }
            }
            "prunable" => {
                if let Some(ref mut wt) = current {
                    wt.prunable = Some(value.unwrap_or_default().to_string());
                }
            }
            _ => {
                // Ignore unknown attributes for forward compatibility
            }
        }
    }

    // Push the last worktree if the output doesn't end with a blank line
    if let Some(wt) = current {
        worktrees.push(finalize_worktree(wt));
    }

    Ok(worktrees)
}

pub(crate) fn parse_local_default_branch(output: &str, remote: &str) -> Result<String, GitError> {
    let trimmed = output.trim();

    // Strip "remote/" prefix if present
    let prefix = format!("{}/", remote);
    let branch = trimmed.strip_prefix(&prefix).unwrap_or(trimmed);

    if branch.is_empty() {
        return Err(GitError::ParseError(format!(
            "Empty branch name from {}/HEAD",
            remote
        )));
    }

    Ok(branch.to_string())
}

pub(crate) fn parse_remote_default_branch(output: &str) -> Result<String, GitError> {
    output
        .lines()
        .find_map(|line| {
            line.strip_prefix("ref: ")
                .and_then(|symref| symref.split_once('\t'))
                .map(|(ref_path, _)| ref_path)
                .and_then(|ref_path| ref_path.strip_prefix("refs/heads/"))
                .map(|branch| branch.to_string())
        })
        .ok_or_else(|| {
            GitError::ParseError("Could not find symbolic ref in ls-remote output".to_string())
        })
}

pub(crate) fn parse_numstat(output: &str) -> Result<(usize, usize), GitError> {
    let mut total_added = 0;
    let mut total_deleted = 0;

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.split('\t');
        let Some(added_str) = parts.next() else {
            continue;
        };
        let Some(deleted_str) = parts.next() else {
            continue;
        };

        // Binary files show "-" for added/deleted
        if added_str == "-" || deleted_str == "-" {
            continue;
        }

        // Skip malformed lines (e.g., missing tabs) by treating parse errors as non-fatal
        let Ok(added) = added_str.parse::<usize>() else {
            continue;
        };
        let Ok(deleted) = deleted_str.parse::<usize>() else {
            continue;
        };

        total_added += added;
        total_deleted += deleted;
    }

    Ok((total_added, total_deleted))
}

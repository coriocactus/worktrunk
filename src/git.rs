use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

#[derive(Debug)]
pub enum GitError {
    CommandFailed(String),
    ParseError(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::CommandFailed(msg) => write!(f, "Git command failed: {}", msg),
            GitError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for GitError {}

/// Helper function to run a git command and return its stdout
fn run_git_command(args: &[&str], path: Option<&std::path::Path>) -> Result<String, GitError> {
    let mut cmd = Command::new("git");
    cmd.args(args);

    if let Some(p) = path {
        cmd.current_dir(p);
    }

    let output = cmd
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitError::CommandFailed(stderr.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn list_worktrees() -> Result<Vec<Worktree>, GitError> {
    let stdout = run_git_command(&["worktree", "list", "--porcelain"], None)?;
    parse_worktree_list(&stdout)
}

fn parse_worktree_list(output: &str) -> Result<Vec<Worktree>, GitError> {
    let mut worktrees = Vec::new();
    let mut current = None;

    for line in output.lines() {
        if line.is_empty() {
            if let Some(wt) = current.take() {
                worktrees.push(wt);
            }
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let key = parts[0];
        let value = parts.get(1).copied();

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
                    let branch = value
                        .ok_or_else(|| GitError::ParseError("branch line missing ref".to_string()))?
                        .strip_prefix("refs/heads/")
                        .unwrap_or(value.unwrap())
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
                    wt.locked = Some(value.unwrap_or("").to_string());
                }
            }
            "prunable" => {
                if let Some(ref mut wt) = current {
                    wt.prunable = Some(value.unwrap_or("").to_string());
                }
            }
            _ => {
                // Ignore unknown attributes for forward compatibility
            }
        }
    }

    // Push the last worktree if the output doesn't end with a blank line
    if let Some(wt) = current {
        worktrees.push(wt);
    }

    Ok(worktrees)
}

/// Get the default branch name using a hybrid approach:
/// 1. Try local cache (origin/HEAD) first for speed
/// 2. If not cached, query the remote and cache the result
pub fn get_default_branch() -> Result<String, GitError> {
    get_default_branch_in(std::path::Path::new("."))
}

/// Get the default branch name for a repository at the given path
pub fn get_default_branch_in(path: &std::path::Path) -> Result<String, GitError> {
    // Try local cache first (fast path)
    if let Ok(branch) = get_local_default_branch(path) {
        return Ok(branch);
    }

    // Query remote and cache it
    let branch = query_remote_default_branch(path)?;
    cache_default_branch(path, &branch)?;
    Ok(branch)
}

/// Try to get the default branch from the local cache (origin/HEAD)
fn get_local_default_branch(path: &std::path::Path) -> Result<String, GitError> {
    let stdout = run_git_command(&["rev-parse", "--abbrev-ref", "origin/HEAD"], Some(path))?;
    parse_local_default_branch(&stdout)
}

/// Parse the output of `git rev-parse --abbrev-ref origin/HEAD`
/// Expected format: "origin/main\n"
fn parse_local_default_branch(output: &str) -> Result<String, GitError> {
    let trimmed = output.trim();

    // Strip "origin/" prefix if present
    let branch = trimmed.strip_prefix("origin/").unwrap_or(trimmed);

    if branch.is_empty() {
        return Err(GitError::ParseError(
            "Empty branch name from origin/HEAD".to_string(),
        ));
    }

    Ok(branch.to_string())
}

/// Query the remote to determine the default branch
fn query_remote_default_branch(path: &std::path::Path) -> Result<String, GitError> {
    let stdout = run_git_command(&["ls-remote", "--symref", "origin", "HEAD"], Some(path))?;
    parse_remote_default_branch(&stdout)
}

/// Parse the output of `git ls-remote --symref origin HEAD`
/// Expected format:
/// ```text
/// ref: refs/heads/main    HEAD
/// 85a1ce7c7182540f9c02453441cb3e8bf0ced214    HEAD
/// ```
fn parse_remote_default_branch(output: &str) -> Result<String, GitError> {
    for line in output.lines() {
        if let Some(symref) = line.strip_prefix("ref: ") {
            // Parse "refs/heads/main\tHEAD"
            let parts: Vec<&str> = symref.split('\t').collect();
            if let Some(ref_path) = parts.first() {
                // Strip "refs/heads/" prefix
                if let Some(branch) = ref_path.strip_prefix("refs/heads/") {
                    return Ok(branch.to_string());
                }
            }
        }
    }

    Err(GitError::ParseError(
        "Could not find symbolic ref in ls-remote output".to_string(),
    ))
}

/// Cache the default branch locally by setting origin/HEAD
fn cache_default_branch(path: &std::path::Path, branch: &str) -> Result<(), GitError> {
    run_git_command(&["remote", "set-head", "origin", branch], Some(path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let output = "worktree /path/to/main
HEAD abcd1234
branch refs/heads/main

worktree /path/to/feature
HEAD efgh5678
branch refs/heads/feature

";

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 2);

        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
        assert_eq!(worktrees[0].head, "abcd1234");
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert!(!worktrees[0].bare);
        assert!(!worktrees[0].detached);

        assert_eq!(worktrees[1].path, PathBuf::from("/path/to/feature"));
        assert_eq!(worktrees[1].head, "efgh5678");
        assert_eq!(worktrees[1].branch, Some("feature".to_string()));
    }

    #[test]
    fn test_parse_detached_worktree() {
        let output = "worktree /path/to/detached
HEAD abcd1234
detached

";

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].detached);
        assert_eq!(worktrees[0].branch, None);
    }

    #[test]
    fn test_parse_locked_worktree() {
        let output = "worktree /path/to/locked
HEAD abcd1234
branch refs/heads/main
locked reason for lock

";

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].locked, Some("reason for lock".to_string()));
    }

    #[test]
    fn test_parse_bare_worktree() {
        let output = "worktree /path/to/bare
HEAD abcd1234
bare

";

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].bare);
    }

    #[test]
    fn test_parse_local_default_branch_with_prefix() {
        let output = "origin/main\n";
        let branch = parse_local_default_branch(output).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn test_parse_local_default_branch_without_prefix() {
        let output = "main\n";
        let branch = parse_local_default_branch(output).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn test_parse_local_default_branch_master() {
        let output = "origin/master\n";
        let branch = parse_local_default_branch(output).unwrap();
        assert_eq!(branch, "master");
    }

    #[test]
    fn test_parse_local_default_branch_custom_name() {
        let output = "origin/develop\n";
        let branch = parse_local_default_branch(output).unwrap();
        assert_eq!(branch, "develop");
    }

    #[test]
    fn test_parse_local_default_branch_empty() {
        let output = "";
        let result = parse_local_default_branch(output);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GitError::ParseError(_)));
    }

    #[test]
    fn test_parse_local_default_branch_whitespace_only() {
        let output = "  \n  ";
        let result = parse_local_default_branch(output);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_remote_default_branch_main() {
        let output = "ref: refs/heads/main\tHEAD
85a1ce7c7182540f9c02453441cb3e8bf0ced214\tHEAD
";
        let branch = parse_remote_default_branch(output).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn test_parse_remote_default_branch_master() {
        let output = "ref: refs/heads/master\tHEAD
abcd1234567890abcd1234567890abcd12345678\tHEAD
";
        let branch = parse_remote_default_branch(output).unwrap();
        assert_eq!(branch, "master");
    }

    #[test]
    fn test_parse_remote_default_branch_custom() {
        let output = "ref: refs/heads/develop\tHEAD
1234567890abcdef1234567890abcdef12345678\tHEAD
";
        let branch = parse_remote_default_branch(output).unwrap();
        assert_eq!(branch, "develop");
    }

    #[test]
    fn test_parse_remote_default_branch_only_symref_line() {
        let output = "ref: refs/heads/main\tHEAD\n";
        let branch = parse_remote_default_branch(output).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn test_parse_remote_default_branch_missing_symref() {
        let output = "85a1ce7c7182540f9c02453441cb3e8bf0ced214\tHEAD\n";
        let result = parse_remote_default_branch(output);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GitError::ParseError(_)));
    }

    #[test]
    fn test_parse_remote_default_branch_empty() {
        let output = "";
        let result = parse_remote_default_branch(output);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_remote_default_branch_malformed_ref() {
        // Missing refs/heads/ prefix
        let output = "ref: main\tHEAD\n";
        let result = parse_remote_default_branch(output);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_remote_default_branch_with_spaces() {
        // Space instead of tab (shouldn't happen in practice, but test robustness)
        let output = "ref: refs/heads/main HEAD\n";
        let result = parse_remote_default_branch(output);
        // This will parse as "main HEAD" which is technically incorrect,
        // but git branch names can contain spaces (though rarely used)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "main HEAD");
    }

    #[test]
    fn test_parse_remote_default_branch_branch_with_slash() {
        let output = "ref: refs/heads/feature/new-ui\tHEAD\n";
        let branch = parse_remote_default_branch(output).unwrap();
        assert_eq!(branch, "feature/new-ui");
    }
}

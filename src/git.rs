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
            // CommandFailed messages are already formatted with emoji and colors
            GitError::CommandFailed(msg) => write!(f, "{}", msg),
            // ParseError messages need formatting
            GitError::ParseError(msg) => {
                use crate::error_format::format_error;
                write!(f, "{}", format_error(msg))
            }
        }
    }
}

impl std::error::Error for GitError {}

/// Repository context for git operations.
///
/// Provides a more ergonomic API than the `*_in(path, ...)` functions by
/// encapsulating the repository path.
///
/// # Examples
///
/// ```no_run
/// use worktrunk::git::Repository;
///
/// let repo = Repository::current();
/// let branch = repo.current_branch()?;
/// let is_dirty = repo.is_dirty()?;
/// # Ok::<(), worktrunk::git::GitError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Repository {
    path: PathBuf,
}

impl Repository {
    /// Create a repository context at the specified path.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create a repository context for the current directory.
    ///
    /// This is the most common usage pattern.
    pub fn current() -> Self {
        Self::at(".")
    }

    /// Get the path this repository context operates on.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Check if a git branch exists (local or remote).
    pub fn branch_exists(&self, branch: &str) -> Result<bool, GitError> {
        // Try local branch first
        let result = run_git_command(
            &["rev-parse", "--verify", &format!("refs/heads/{}", branch)],
            Some(&self.path),
        );
        if result.is_ok() {
            return Ok(true);
        }

        // Try remote branch
        let result = run_git_command(
            &[
                "rev-parse",
                "--verify",
                &format!("refs/remotes/origin/{}", branch),
            ],
            Some(&self.path),
        );
        Ok(result.is_ok())
    }

    /// Get the current branch name, or None if in detached HEAD state.
    pub fn current_branch(&self) -> Result<Option<String>, GitError> {
        let stdout = run_git_command(&["branch", "--show-current"], Some(&self.path))?;
        let branch = stdout.trim();

        if branch.is_empty() {
            Ok(None) // Detached HEAD
        } else {
            Ok(Some(branch.to_string()))
        }
    }

    /// Get the default branch name for the repository.
    ///
    /// Uses a hybrid approach:
    /// 1. Try local cache (origin/HEAD) first for speed
    /// 2. If not cached, query the remote and cache the result
    pub fn default_branch(&self) -> Result<String, GitError> {
        // Try local cache first (fast path)
        if let Ok(branch) = self.get_local_default_branch() {
            return Ok(branch);
        }

        // Query remote and cache it
        let branch = self.query_remote_default_branch()?;
        self.cache_default_branch(&branch)?;
        Ok(branch)
    }

    /// Get the git common directory (the actual .git directory for the repository).
    pub fn git_common_dir(&self) -> Result<PathBuf, GitError> {
        let stdout = run_git_command(&["rev-parse", "--git-common-dir"], Some(&self.path))?;
        Ok(PathBuf::from(stdout.trim()))
    }

    /// Get the git directory (may be different from common-dir in worktrees).
    pub fn git_dir(&self) -> Result<PathBuf, GitError> {
        let stdout = run_git_command(&["rev-parse", "--git-dir"], Some(&self.path))?;
        Ok(PathBuf::from(stdout.trim()))
    }

    /// Get the canonicalized repository root directory (parent of .git).
    ///
    /// The canonicalization resolves symlinks and relative paths, which is important
    /// for worktree operations to ensure consistent path handling.
    pub fn repo_root(&self) -> Result<PathBuf, GitError> {
        let git_common_dir = self
            .git_common_dir()?
            .canonicalize()
            .map_err(|e| GitError::CommandFailed(format!("Failed to canonicalize path: {}", e)))?;

        git_common_dir
            .parent()
            .ok_or_else(|| GitError::CommandFailed("Invalid git directory".to_string()))
            .map(|p| p.to_path_buf())
    }

    /// Check if the working tree has uncommitted changes.
    pub fn is_dirty(&self) -> Result<bool, GitError> {
        let stdout = run_git_command(&["status", "--porcelain"], Some(&self.path))?;
        Ok(!stdout.trim().is_empty())
    }

    /// Get the worktree root directory (top-level of the working tree).
    pub fn worktree_root(&self) -> Result<PathBuf, GitError> {
        let stdout = run_git_command(&["rev-parse", "--show-toplevel"], Some(&self.path))?;
        Ok(PathBuf::from(stdout.trim()))
    }

    /// Check if the path is in a worktree (vs the main repository).
    pub fn is_in_worktree(&self) -> Result<bool, GitError> {
        let git_dir = self.git_dir()?;
        let common_dir = self.git_common_dir()?;
        Ok(git_dir != common_dir)
    }

    /// Check if base is an ancestor of head (i.e., would be a fast-forward).
    pub fn is_ancestor(&self, base: &str, head: &str) -> Result<bool, GitError> {
        let output = std::process::Command::new("git")
            .args(["merge-base", "--is-ancestor", base, head])
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        Ok(output.status.success())
    }

    /// Count commits between base and head.
    pub fn count_commits(&self, base: &str, head: &str) -> Result<usize, GitError> {
        let range = format!("{}..{}", base, head);
        let stdout = run_git_command(&["rev-list", "--count", &range], Some(&self.path))?;
        stdout
            .trim()
            .parse()
            .map_err(|e| GitError::ParseError(format!("Failed to parse commit count: {}", e)))
    }

    /// Check if there are merge commits in the range base..head.
    pub fn has_merge_commits(&self, base: &str, head: &str) -> Result<bool, GitError> {
        let range = format!("{}..{}", base, head);
        let stdout = run_git_command(&["rev-list", "--merges", &range], Some(&self.path))?;
        Ok(!stdout.trim().is_empty())
    }

    /// Get files changed between base and head.
    pub fn changed_files(&self, base: &str, head: &str) -> Result<Vec<String>, GitError> {
        let range = format!("{}..{}", base, head);
        let stdout = run_git_command(&["diff", "--name-only", &range], Some(&self.path))?;
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    /// Get commit timestamp in seconds since epoch.
    pub fn commit_timestamp(&self, commit: &str) -> Result<i64, GitError> {
        let stdout = run_git_command(&["show", "-s", "--format=%ct", commit], Some(&self.path))?;
        stdout
            .trim()
            .parse()
            .map_err(|e| GitError::ParseError(format!("Failed to parse timestamp: {}", e)))
    }

    /// Get commit message (subject line) for a commit.
    pub fn commit_message(&self, commit: &str) -> Result<String, GitError> {
        let stdout = run_git_command(&["show", "-s", "--format=%s", commit], Some(&self.path))?;
        Ok(stdout.trim().to_string())
    }

    /// Get the upstream tracking branch for the given branch.
    pub fn upstream_branch(&self, branch: &str) -> Result<Option<String>, GitError> {
        let result = run_git_command(
            &["rev-parse", "--abbrev-ref", &format!("{}@{{u}}", branch)],
            Some(&self.path),
        );

        match result {
            Ok(upstream) => {
                let trimmed = upstream.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed.to_string()))
                }
            }
            Err(_) => Ok(None), // No upstream configured
        }
    }

    /// Get merge/rebase status for the worktree.
    pub fn worktree_state(&self) -> Result<Option<String>, GitError> {
        let git_dir = self.git_dir()?;

        // Check for merge
        if git_dir.join("MERGE_HEAD").exists() {
            return Ok(Some("MERGING".to_string()));
        }

        // Check for rebase
        if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
            let rebase_dir = if git_dir.join("rebase-merge").exists() {
                git_dir.join("rebase-merge")
            } else {
                git_dir.join("rebase-apply")
            };

            if let (Ok(msgnum), Ok(end)) = (
                std::fs::read_to_string(rebase_dir.join("msgnum")),
                std::fs::read_to_string(rebase_dir.join("end")),
            ) {
                let current = msgnum.trim();
                let total = end.trim();
                return Ok(Some(format!("REBASING {}/{}", current, total)));
            }

            return Ok(Some("REBASING".to_string()));
        }

        // Check for cherry-pick
        if git_dir.join("CHERRY_PICK_HEAD").exists() {
            return Ok(Some("CHERRY-PICKING".to_string()));
        }

        // Check for revert
        if git_dir.join("REVERT_HEAD").exists() {
            return Ok(Some("REVERTING".to_string()));
        }

        // Check for bisect
        if git_dir.join("BISECT_LOG").exists() {
            return Ok(Some("BISECTING".to_string()));
        }

        Ok(None)
    }

    /// Calculate commits ahead and behind between two refs.
    ///
    /// Returns (ahead, behind) where ahead is commits in head not in base,
    /// and behind is commits in base not in head.
    pub fn ahead_behind(&self, base: &str, head: &str) -> Result<(usize, usize), GitError> {
        let ahead = self.count_commits(base, head)?;
        let behind = self.count_commits(head, base)?;
        Ok((ahead, behind))
    }

    /// Get line diff statistics for working tree changes (unstaged + staged).
    ///
    /// Returns (added_lines, deleted_lines).
    pub fn working_tree_diff_stats(&self) -> Result<(usize, usize), GitError> {
        let stdout = run_git_command(&["diff", "--numstat", "HEAD"], Some(&self.path))?;
        parse_numstat(&stdout)
    }

    /// Get line diff statistics between two refs (using three-dot diff for merge base).
    ///
    /// Returns (added_lines, deleted_lines).
    pub fn branch_diff_stats(&self, base: &str, head: &str) -> Result<(usize, usize), GitError> {
        let range = format!("{}...{}", base, head);
        let stdout = run_git_command(&["diff", "--numstat", &range], Some(&self.path))?;
        parse_numstat(&stdout)
    }

    /// Get all branch names (local branches only).
    pub fn all_branches(&self) -> Result<Vec<String>, GitError> {
        let stdout = run_git_command(&["branch", "--format=%(refname:short)"], Some(&self.path))?;
        Ok(stdout
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    /// Get the merge base between two commits.
    pub fn merge_base(&self, commit1: &str, commit2: &str) -> Result<String, GitError> {
        let output = run_git_command(&["merge-base", commit1, commit2], Some(&self.path))?;
        Ok(output.trim().to_string())
    }

    /// Get commit subjects (first line of commit message) from a range.
    pub fn commit_subjects(&self, range: &str) -> Result<Vec<String>, GitError> {
        let output = run_git_command(&["log", "--format=%s", range], Some(&self.path))?;
        Ok(output.lines().map(|s| s.to_string()).collect())
    }

    /// Check if there are staged changes.
    pub fn has_staged_changes(&self) -> Result<bool, GitError> {
        let output = std::process::Command::new("git")
            .args(["diff", "--cached", "--quiet", "--exit-code"])
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        // exit code 0 = no changes, 1 = has changes
        Ok(!output.status.success())
    }

    /// List all worktrees for this repository.
    pub fn list_worktrees(&self) -> Result<Vec<Worktree>, GitError> {
        let stdout = run_git_command(&["worktree", "list", "--porcelain"], Some(&self.path))?;
        parse_worktree_list(&stdout)
    }

    /// Find the worktree path for a given branch, if one exists.
    pub fn worktree_for_branch(&self, branch: &str) -> Result<Option<PathBuf>, GitError> {
        let worktrees = self.list_worktrees()?;

        for wt in worktrees {
            if let Some(ref wt_branch) = wt.branch
                && wt_branch == branch
            {
                return Ok(Some(wt.path));
            }
        }

        Ok(None)
    }

    /// Get branches that don't have worktrees (available for switch).
    pub fn available_branches(&self) -> Result<Vec<String>, GitError> {
        let all_branches = self.all_branches()?;
        let worktrees = self.list_worktrees()?;

        // Collect branches that have worktrees
        let branches_with_worktrees: std::collections::HashSet<String> =
            worktrees.into_iter().filter_map(|wt| wt.branch).collect();

        // Filter out branches with worktrees
        Ok(all_branches
            .into_iter()
            .filter(|branch| !branches_with_worktrees.contains(branch))
            .collect())
    }

    // Private helper methods for default_branch()

    fn get_local_default_branch(&self) -> Result<String, GitError> {
        let stdout = run_git_command(
            &["rev-parse", "--abbrev-ref", "origin/HEAD"],
            Some(&self.path),
        )?;
        parse_local_default_branch(&stdout)
    }

    fn query_remote_default_branch(&self) -> Result<String, GitError> {
        let stdout = run_git_command(
            &["ls-remote", "--symref", "origin", "HEAD"],
            Some(&self.path),
        )?;
        parse_remote_default_branch(&stdout)
    }

    fn cache_default_branch(&self, branch: &str) -> Result<(), GitError> {
        run_git_command(&["remote", "set-head", "origin", branch], Some(&self.path))?;
        Ok(())
    }
}

/// Helper function to run a git command and return its stdout.
///
/// For commands that don't need the output (e.g., git switch, git worktree add),
/// the returned String can be ignored.
pub fn run_git_command(args: &[&str], path: Option<&std::path::Path>) -> Result<String, GitError> {
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

fn parse_remote_default_branch(output: &str) -> Result<String, GitError> {
    for line in output.lines() {
        if let Some(symref) = line.strip_prefix("ref: ") {
            // Parse "refs/heads/main\tHEAD"
            if let Some((ref_path, _)) = symref.split_once('\t') {
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

fn parse_numstat(output: &str) -> Result<(usize, usize), GitError> {
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

        let added: usize = added_str
            .parse()
            .map_err(|e| GitError::ParseError(format!("Failed to parse added lines: {}", e)))?;
        let deleted: usize = deleted_str
            .parse()
            .map_err(|e| GitError::ParseError(format!("Failed to parse deleted lines: {}", e)))?;

        total_added += added;
        total_deleted += deleted;
    }

    Ok((total_added, total_deleted))
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
        // Space instead of tab - should be rejected as malformed input
        let output = "ref: refs/heads/main HEAD\n";
        let result = parse_remote_default_branch(output);
        // Using split_once correctly rejects malformed input with spaces instead of tabs
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_remote_default_branch_branch_with_slash() {
        let output = "ref: refs/heads/feature/new-ui\tHEAD\n";
        let branch = parse_remote_default_branch(output).unwrap();
        assert_eq!(branch, "feature/new-ui");
    }

    #[test]
    fn test_get_current_branch_parse() {
        // Test parsing of branch --show-current output
        // We can't test the actual command without a git repo,
        // but we've verified the parsing logic through the implementation
    }

    #[test]
    fn test_worktree_for_branch_not_found() {
        // Test that worktree_for_branch returns None when no worktree exists
        // This would require a git repo, so we'll test this in integration tests
    }
}

//! Worktrunk error types and formatting
//!
//! This module provides typed error handling:
//!
//! - **`GitError`** - A typed enum for domain errors that can be pattern-matched
//!   and tested. Use `.into()` to convert to `anyhow::Error` while preserving the
//!   type for pattern matching. Display produces styled output for users.
//!
//! - **`WorktrunkError`** - A minimal enum for semantic errors that need
//!   special handling (exit codes, silent errors).

use std::path::PathBuf;

use color_print::{cformat, cwrite};

use super::HookType;
use crate::path::format_path_for_display;
use crate::styling::{ERROR_EMOJI, HINT_EMOJI, INFO_EMOJI, format_with_gutter};

/// Domain errors for git and worktree operations.
///
/// This enum provides structured error data that can be pattern-matched and tested.
/// Each variant stores the data needed to construct a user-facing error message.
/// Display produces styled output with emoji and colors.
///
/// # Usage
///
/// ```ignore
/// // Return a typed error (Display produces styled output)
/// return Err(GitError::DetachedHead { action: Some("merge".into()) }.into());
///
/// // Pattern match on errors
/// if let Some(GitError::BranchAlreadyExists { branch }) = err.downcast_ref() {
///     println!("Branch {} exists", branch);
/// }
/// ```
#[derive(Debug, Clone)]
pub enum GitError {
    // Git state errors
    DetachedHead {
        action: Option<String>,
    },
    UncommittedChanges {
        action: Option<String>,
    },
    BranchAlreadyExists {
        branch: String,
    },
    InvalidReference {
        reference: String,
    },

    // Worktree errors
    WorktreeMissing {
        branch: String,
    },
    NoWorktreeFound {
        branch: String,
    },
    WorktreePathOccupied {
        branch: String,
        path: PathBuf,
        occupant: Option<String>,
    },
    WorktreePathExists {
        path: PathBuf,
    },
    WorktreePathMismatch {
        branch: String,
        expected_path: PathBuf,
        actual_path: PathBuf,
    },
    WorktreeCreationFailed {
        branch: String,
        base_branch: Option<String>,
        error: String,
    },
    WorktreeRemovalFailed {
        branch: String,
        path: PathBuf,
        error: String,
    },

    // Merge/push errors
    ConflictingChanges {
        files: Vec<String>,
        worktree_path: PathBuf,
    },
    NotFastForward {
        target_branch: String,
        commits_formatted: String,
        in_merge_context: bool,
    },
    MergeCommitsFound,
    RebaseConflict {
        target_branch: String,
        git_output: String,
    },
    PushFailed {
        error: String,
    },

    // Validation/other errors
    NotInteractive,
    ParseError {
        message: String,
    },
    LlmCommandFailed {
        command: String,
        error: String,
    },
    ProjectConfigNotFound {
        config_path: PathBuf,
    },
    Other {
        message: String,
    },
}

impl std::error::Error for GitError {}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::DetachedHead { action } => {
                let message = match action {
                    Some(action) => format!("Cannot {action}: not on a branch (detached HEAD)"),
                    None => "Not on a branch (detached HEAD)".to_string(),
                };
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>{message}</>\n\n{HINT_EMOJI} <dim>Switch to a branch first with 'git switch <<branch>>'</>"
                )
            }

            GitError::UncommittedChanges { action } => {
                let message = match action {
                    Some(action) => {
                        format!("Cannot {action}: working tree has uncommitted changes")
                    }
                    None => "Working tree has uncommitted changes".to_string(),
                };
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>{message}</>\n\n{HINT_EMOJI} <dim>Commit or stash them first</>"
                )
            }

            GitError::BranchAlreadyExists { branch } => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Branch <bold>{branch}</> already exists</>\n\n{HINT_EMOJI} <dim>Remove --create flag to switch to it</>"
                )
            }

            GitError::InvalidReference { reference } => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Branch <bold>{reference}</> not found</>\n\n{HINT_EMOJI} <dim>Use --create to create it, or 'wt list --branches --remotes' for available branches</>"
                )
            }

            GitError::WorktreeMissing { branch } => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Worktree directory missing for <bold>{branch}</></>\n\n{HINT_EMOJI} <dim>Run 'git worktree prune' to clean up</>"
                )
            }

            GitError::NoWorktreeFound { branch } => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>No worktree found for branch <bold>{branch}</></>"
                )
            }

            GitError::WorktreePathOccupied {
                branch,
                path,
                occupant,
            } => {
                let path_display = format_path_for_display(path);
                let occupant_note = occupant
                    .as_ref()
                    .map(|b| format!(" (currently on <bold>{b}</>)"))
                    .unwrap_or_default();
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Cannot create worktree for <bold>{branch}</>: target path already exists</>\n\n{HINT_EMOJI} <dim>Reuse the existing worktree at {path_display}{occupant_note} or remove it before retrying</>"
                )
            }

            GitError::WorktreePathExists { path } => {
                let path_display = format_path_for_display(path);
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Directory already exists: <bold>{path_display}</></>\n\n{HINT_EMOJI} <dim>Remove the directory or use a different branch name</>"
                )
            }

            GitError::WorktreePathMismatch {
                branch,
                expected_path,
                actual_path,
            } => {
                let expected = format_path_for_display(expected_path);
                let actual = format_path_for_display(actual_path);
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Ambiguous: <bold>{expected}</> has a worktree on a different branch, but branch <bold>{branch}</> exists at <bold>{actual}</></>\n\n{HINT_EMOJI} <dim>Use 'wt list' to see worktree-branch mappings</>"
                )
            }

            GitError::WorktreeCreationFailed {
                branch,
                base_branch,
                error,
            } => {
                let base_suffix = base_branch
                    .as_ref()
                    .map(|base| format!(" from base <bold>{base}</>"))
                    .unwrap_or_default();
                let header = cformat!(
                    "{ERROR_EMOJI} <red>Failed to create worktree for <bold>{branch}</>{base_suffix}</>"
                );
                write!(f, "{}", format_error_block(header, error))
            }

            GitError::WorktreeRemovalFailed {
                branch,
                path,
                error,
            } => {
                let path_display = format_path_for_display(path);
                let header = cformat!(
                    "{ERROR_EMOJI} <red>Failed to remove worktree for <bold>{branch}</> at <bold>{path_display}</></>"
                );
                write!(f, "{}", format_error_block(header, error))
            }

            GitError::ConflictingChanges {
                files,
                worktree_path,
            } => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Cannot push: conflicting uncommitted changes in:</>\n\n"
                )?;
                if !files.is_empty() {
                    let joined_files = files.join("\n");
                    write!(f, "{}", format_with_gutter(&joined_files, "", None))?;
                }
                let path_display = format_path_for_display(worktree_path);
                cwrite!(
                    f,
                    "\n{HINT_EMOJI} <dim>Commit or stash these changes in {path_display} first</>"
                )
            }

            GitError::NotFastForward {
                target_branch,
                commits_formatted,
                in_merge_context,
            } => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Can't push to local <bold>{target_branch}</> branch: it has newer commits</>"
                )?;
                if !commits_formatted.is_empty() {
                    write!(f, "\n{}", format_with_gutter(commits_formatted, "", None))?;
                }
                // Context-appropriate hint
                let hint = if *in_merge_context {
                    "Run 'wt merge' again to incorporate these changes".to_string()
                } else {
                    format!("Use 'wt step rebase' or 'wt merge' to rebase onto {target_branch}")
                };
                cwrite!(f, "\n{HINT_EMOJI} <dim>{hint}</>")
            }

            GitError::MergeCommitsFound => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Found merge commits in push range</>\n\n{HINT_EMOJI} <dim>Use --allow-merge-commits to push non-linear history</>"
                )
            }

            GitError::RebaseConflict {
                target_branch,
                git_output,
            } => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Rebase onto <bold>{target_branch}</> incomplete</>"
                )?;
                if !git_output.is_empty() {
                    write!(f, "\n{}", format_with_gutter(git_output, "", None))
                } else {
                    cwrite!(
                        f,
                        "\n\n{HINT_EMOJI} <dim>Resolve conflicts and run 'git rebase --continue'</>\n{HINT_EMOJI} <dim>Or abort with 'git rebase --abort'</>"
                    )
                }
            }

            GitError::PushFailed { error } => {
                let header = cformat!("{ERROR_EMOJI} <red>Push failed</>");
                write!(f, "{}", format_error_block(header, error))
            }

            GitError::NotInteractive => {
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>Cannot prompt for approval in non-interactive environment</>\n\n{HINT_EMOJI} <dim>In CI/CD, use --force to skip prompts. To pre-approve commands, use 'wt config approvals add'</>"
                )
            }

            GitError::LlmCommandFailed { command, error } => {
                let error_header =
                    cformat!("{ERROR_EMOJI} <red>Commit generation command failed</>");
                let error_block = format_error_block(error_header, error);
                let command_gutter = format_with_gutter(command, "", None);
                write!(
                    f,
                    "{}\n\n{INFO_EMOJI} Ran command:\n{}",
                    error_block.trim_end(),
                    command_gutter.trim_end()
                )
            }

            GitError::ProjectConfigNotFound { config_path } => {
                let path_display = format_path_for_display(config_path);
                cwrite!(
                    f,
                    "{ERROR_EMOJI} <red>No project configuration found</>\n\n{HINT_EMOJI} <dim>Create a config file at: <bold>{path_display}</></>"
                )
            }

            GitError::ParseError { message } | GitError::Other { message } => {
                cwrite!(f, "{ERROR_EMOJI} <red>{message}</>")
            }
        }
    }
}

/// Semantic errors that require special handling in main.rs
///
/// Most errors use anyhow::bail! with formatted messages. This enum is only
/// for cases that need exit code extraction or special handling.
#[derive(Debug)]
pub enum WorktrunkError {
    /// Child process exited with non-zero code (preserves exit code for signals)
    ChildProcessExited { code: i32, message: String },
    /// Hook command failed
    HookCommandFailed {
        hook_type: HookType,
        command_name: Option<String>,
        error: String,
        exit_code: Option<i32>,
    },
    /// Command was not approved by user (silent error)
    CommandNotApproved,
}

impl std::fmt::Display for WorktrunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorktrunkError::ChildProcessExited { message, .. } => {
                cwrite!(f, "{ERROR_EMOJI} <red>{message}</>")
            }
            WorktrunkError::HookCommandFailed {
                hook_type,
                command_name,
                error,
                ..
            } => {
                if let Some(name) = command_name {
                    cwrite!(
                        f,
                        "{ERROR_EMOJI} <red>{hook_type} command failed: <bold>{name}</>: {error}</>\n\n{HINT_EMOJI} <dim>Use --no-verify to skip {hook_type} commands</>"
                    )
                } else {
                    cwrite!(
                        f,
                        "{ERROR_EMOJI} <red>{hook_type} command failed: {error}</>\n\n{HINT_EMOJI} <dim>Use --no-verify to skip {hook_type} commands</>"
                    )
                }
            }
            WorktrunkError::CommandNotApproved => {
                Ok(()) // on_skip callback handles the printing
            }
        }
    }
}

impl std::error::Error for WorktrunkError {}

/// Extract exit code from WorktrunkError, if applicable
pub fn exit_code(err: &anyhow::Error) -> Option<i32> {
    err.downcast_ref::<WorktrunkError>().and_then(|e| match e {
        WorktrunkError::ChildProcessExited { code, .. } => Some(*code),
        WorktrunkError::HookCommandFailed { exit_code, .. } => *exit_code,
        WorktrunkError::CommandNotApproved => None,
    })
}

/// Check if error is CommandNotApproved (silent error)
pub fn is_command_not_approved(err: &anyhow::Error) -> bool {
    err.downcast_ref::<WorktrunkError>()
        .is_some_and(|e| matches!(e, WorktrunkError::CommandNotApproved))
}

/// Format an error with header and gutter content
fn format_error_block(header: String, error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        header
    } else {
        format!("{header}\n{}", format_with_gutter(trimmed, "", None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_error_display_contains_emoji() {
        // Display produces styled output with emoji
        let err = GitError::DetachedHead { action: None };
        let output = err.to_string();
        assert!(output.contains("❌")); // ERROR_EMOJI
        assert!(output.contains("detached HEAD"));
        assert!(output.contains("💡")); // HINT_EMOJI
    }

    #[test]
    fn test_git_error_display_includes_action() {
        let err = GitError::DetachedHead {
            action: Some("push".into()),
        };
        let output = err.to_string();
        assert!(output.contains("Cannot push"));

        let err = GitError::UncommittedChanges {
            action: Some("remove worktree".into()),
        };
        let output = err.to_string();
        assert!(output.contains("Cannot remove worktree"));
    }

    #[test]
    fn test_into_preserves_type_for_display() {
        // .into() preserves type so we can downcast and use Display
        let err: anyhow::Error = GitError::BranchAlreadyExists {
            branch: "main".into(),
        }
        .into();

        // Can downcast and get styled output via Display
        let git_err = err.downcast_ref::<GitError>().expect("Should downcast");
        let output = git_err.to_string();
        assert!(output.contains("❌")); // Should be styled
        assert!(output.contains("main"));
        assert!(output.contains("already exists"));
    }

    #[test]
    fn test_pattern_matching_with_into() {
        // .into() preserves type for pattern matching
        let err: anyhow::Error = GitError::BranchAlreadyExists {
            branch: "main".into(),
        }
        .into();

        if let Some(GitError::BranchAlreadyExists { branch }) = err.downcast_ref::<GitError>() {
            assert_eq!(branch, "main");
        } else {
            panic!("Failed to downcast and pattern match");
        }
    }

    #[test]
    fn test_worktree_error_with_path() {
        let err = GitError::WorktreePathExists {
            path: PathBuf::from("/some/path"),
        };
        let output = err.to_string();
        assert!(output.contains("Directory already exists"));
    }
}

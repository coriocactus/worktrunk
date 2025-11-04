//! Git error types and formatting

use std::path::PathBuf;

use super::HookType;

#[derive(Debug)]
pub enum GitError {
    /// Generic error with a message
    CommandFailed(String),
    /// Error for parsing failures
    ParseError(String),
    /// Repository is in detached HEAD state
    DetachedHead,
    /// Working tree has untracked files
    UntrackedFiles,
    /// Hook command failed
    HookCommandFailed {
        hook_type: HookType,
        command_name: Option<String>,
        error: String,
    },
    /// Working tree has uncommitted changes
    UncommittedChanges,
    /// Branch already exists (when trying to create)
    BranchAlreadyExists { branch: String },
    /// Worktree directory is missing
    WorktreeMissing { branch: String },
    /// No worktree found for branch
    NoWorktreeFound { branch: String },
    /// Cannot push due to conflicting uncommitted changes
    ConflictingChanges {
        files: Vec<String>,
        worktree_path: PathBuf,
    },
    /// Push is not a fast-forward
    NotFastForward {
        target_branch: String,
        commits_formatted: String,
        files_formatted: String,
    },
    /// Found merge commits in push range
    MergeCommitsFound,
    /// Command was not approved by user
    CommandNotApproved,
    /// Push operation failed
    PushFailed { error: String },
    /// Rebase resulted in a conflict or incomplete state
    RebaseConflict {
        state: String,
        target_branch: String,
        git_output: String,
    },
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::styling::{ERROR, ERROR_EMOJI, HINT, HINT_EMOJI};

        match self {
            // Plain message that will be displayed by main.rs
            GitError::CommandFailed(msg) => write!(f, "{}", msg),

            // ParseError messages need formatting
            GitError::ParseError(msg) => {
                write!(f, "{ERROR_EMOJI} {ERROR}{msg}{ERROR:#}")
            }

            // Detached HEAD error
            GitError::DetachedHead => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Not on a branch (detached HEAD){ERROR:#}\n\n{HINT_EMOJI} {HINT}You are in detached HEAD state{HINT:#}"
                )
            }

            // Untracked files error
            GitError::UntrackedFiles => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Working tree has untracked files{ERROR:#}\n\n{HINT_EMOJI} {HINT}Add them with 'git add' and try again{HINT:#}"
                )
            }

            // Hook command failed
            GitError::HookCommandFailed {
                hook_type,
                command_name,
                error,
            } => {
                let error_bold = ERROR.bold();
                match command_name {
                    Some(name) => write!(
                        f,
                        "{ERROR_EMOJI} {ERROR}{hook_type} command failed: {error_bold}{name}{error_bold:#}{ERROR:#}\n\n{error}\n\n{HINT_EMOJI} {HINT}Use --no-hooks to skip {hook_type} commands{HINT:#}"
                    ),
                    None => write!(
                        f,
                        "{ERROR_EMOJI} {ERROR}{hook_type} command failed{ERROR:#}\n\n{error}\n\n{HINT_EMOJI} {HINT}Use --no-hooks to skip {hook_type} commands{HINT:#}"
                    ),
                }
            }

            // Uncommitted changes
            GitError::UncommittedChanges => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Working tree has uncommitted changes{ERROR:#}\n\n{HINT_EMOJI} {HINT}Commit or stash them first{HINT:#}"
                )
            }

            // Branch already exists
            GitError::BranchAlreadyExists { branch } => {
                let error_bold = ERROR.bold();
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Branch {error_bold}{branch}{error_bold:#}{ERROR} already exists{ERROR:#}\n\n{HINT_EMOJI} {HINT}Remove --create flag to switch to it{HINT:#}"
                )
            }

            // Worktree missing
            GitError::WorktreeMissing { branch } => {
                let error_bold = ERROR.bold();
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Worktree directory missing for {error_bold}{branch}{error_bold:#}{ERROR:#}\n\n{HINT_EMOJI} {HINT}Run 'git worktree prune' to clean up{HINT:#}"
                )
            }

            // No worktree found
            GitError::NoWorktreeFound { branch } => {
                let error_bold = ERROR.bold();
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}No worktree found for branch {error_bold}{branch}{error_bold:#}{ERROR:#}"
                )
            }

            // Conflicting changes
            GitError::ConflictingChanges {
                files,
                worktree_path,
            } => {
                use crate::styling::AnstyleStyle;
                let dim = AnstyleStyle::new().dimmed();

                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Cannot push: conflicting uncommitted changes in:{ERROR:#}\n\n"
                )?;
                for file in files {
                    writeln!(f, "{dim}•{dim:#} {file}")?;
                }
                write!(
                    f,
                    "\n{HINT_EMOJI} {HINT}Commit or stash these changes in {} first{HINT:#}",
                    worktree_path.display()
                )
            }

            // Not fast-forward
            GitError::NotFastForward {
                target_branch,
                commits_formatted,
                files_formatted,
            } => {
                let error_bold = ERROR.bold();

                writeln!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Can't push to local {error_bold}{target_branch}{error_bold:#} branch: it has newer commits{ERROR:#}"
                )?;

                // Show the formatted commit log
                if !commits_formatted.is_empty() {
                    writeln!(f)?;
                    write!(f, "{}", commits_formatted)?;
                }

                // Show the formatted diff stat
                if !files_formatted.is_empty() {
                    writeln!(f)?;
                    write!(f, "{}", files_formatted)?;
                }

                write!(
                    f,
                    "\n{HINT_EMOJI} {HINT}Use 'wt merge' to rebase your changes onto {target_branch}{HINT:#}"
                )
            }

            // Merge commits found
            GitError::MergeCommitsFound => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Found merge commits in push range{ERROR:#}\n\n{HINT_EMOJI} {HINT}Use --allow-merge-commits to push non-linear history{HINT:#}"
                )
            }

            // Command not approved
            GitError::CommandNotApproved => {
                Ok(()) // on_skip callback handles the printing
            }

            // Push failed
            GitError::PushFailed { error } => {
                write!(f, "{ERROR_EMOJI} {ERROR}Push failed: {error}{ERROR:#}")
            }

            // Rebase conflict
            GitError::RebaseConflict {
                state: _,
                target_branch,
                git_output,
            } => {
                use crate::styling::format_with_gutter;
                let error_bold = ERROR.bold();

                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Rebase onto {error_bold}{target_branch}{error_bold:#}{ERROR} incomplete{ERROR:#}"
                )?;

                if !git_output.is_empty() {
                    writeln!(f)?;
                    write!(f, "{}", format_with_gutter(git_output, "", None))?;
                } else {
                    // Fallback hints if no git output (edge case)
                    write!(
                        f,
                        "\n\n{HINT_EMOJI} {HINT}Resolve conflicts and run 'git rebase --continue'{HINT:#}\n{HINT_EMOJI} {HINT}Or abort with 'git rebase --abort'{HINT:#}"
                    )?;
                }

                Ok(())
            }
        }
    }
}

impl std::error::Error for GitError {}

// Automatic conversion from io::Error to GitError
// This eliminates the need for manual .map_err() on output functions
impl From<std::io::Error> for GitError {
    fn from(e: std::io::Error) -> Self {
        GitError::CommandFailed(e.to_string())
    }
}

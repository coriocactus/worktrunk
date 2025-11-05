use skim::prelude::*;
use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use worktrunk::config::WorktrunkConfig;
use worktrunk::git::{GitError, GitResultExt, Repository};

use super::list::model::{ListItem, gather_list_data};
use super::worktree::handle_switch;
use crate::output::handle_switch_output;

/// Preview modes for the interactive selector
///
/// Each mode shows a different aspect of the worktree:
/// 1. WorkingTree: Uncommitted changes (git diff HEAD --stat)
/// 2. History: Commit history since diverging from main (git log with merge-base)
/// 3. BranchDiff: Line diffs in commits ahead of main (git diff --stat main…)
///
/// Loosely aligned with `wt list` columns, though not a perfect match:
/// - Mode 1 corresponds to "HEAD±" column
/// - Mode 2 shows commits (related to "main↕" counts)
/// - Mode 3 corresponds to "main…± (--full)" column
///
/// Note: Order of modes 2 & 3 could potentially be swapped
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewMode {
    WorkingTree = 1,
    History = 2,
    BranchDiff = 3,
}

impl PreviewMode {
    fn from_u8(n: u8) -> Self {
        match n {
            2 => Self::History,
            3 => Self::BranchDiff,
            _ => Self::WorkingTree,
        }
    }

    fn read_from_state() -> Self {
        let state_path = Self::state_path();
        fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(Self::from_u8)
            .unwrap_or(Self::WorkingTree)
    }

    fn state_path() -> PathBuf {
        // Use per-process temp file to avoid race conditions when running multiple instances
        std::env::temp_dir().join(format!("wt-select-mode-{}", std::process::id()))
    }
}

/// Cached pager configuration to avoid repeated detection
static PAGER_CONFIG: OnceLock<Option<String>> = OnceLock::new();

/// Get cached pager configuration, detecting on first call
fn get_pager_config() -> &'static Option<String> {
    PAGER_CONFIG.get_or_init(detect_pager)
}

/// Detect configured diff renderer (colorizer) for preview output
///
/// Respects user's git pager configuration, but treats the tool as a
/// non-interactive renderer (not a pager) in the preview context.
///
/// Priority order:
/// 1. GIT_PAGER environment variable (git's own preference)
/// 2. git config pager.diff or core.pager
/// 3. PAGER environment variable (system default)
/// 4. None (fallback to plain colored output)
///
/// Returns the renderer command string to be executed via shell
fn detect_pager() -> Option<String> {
    let repo = Repository::current();

    // 1. Check GIT_PAGER (highest priority - user's explicit choice)
    if let Ok(git_pager) = std::env::var("GIT_PAGER") {
        let trimmed = git_pager.trim();
        if !trimmed.is_empty() && trimmed != "cat" {
            return Some(trimmed.to_string());
        }
    }

    // 2. Check git config (pager.diff is more specific than core.pager)
    if let Ok(pager_diff) = repo.run_command(&["config", "--get", "pager.diff"]) {
        let trimmed = pager_diff.trim();
        if !trimmed.is_empty() && trimmed != "cat" {
            return Some(trimmed.to_string());
        }
    }

    if let Ok(core_pager) = repo.run_command(&["config", "--get", "core.pager"]) {
        let trimmed = core_pager.trim();
        if !trimmed.is_empty() && trimmed != "cat" {
            return Some(trimmed.to_string());
        }
    }

    // 3. Check PAGER environment variable
    if let Ok(pager) = std::env::var("PAGER") {
        let trimmed = pager.trim();
        if !trimmed.is_empty() && trimmed != "cat" {
            return Some(trimmed.to_string());
        }
    }

    // 4. No renderer configured - return None to use plain colored output
    None
}

/// Run git diff through configured renderer (colorizer), or fall back to --color=always
///
/// The renderer is run in non-interactive mode (via environment variables) suitable
/// for embedding in a TUI preview pane. Interactive paging features are disabled.
fn run_diff_with_pager(repo: &Repository, args: &[&str]) -> Result<String, GitError> {
    // First get git output with color
    let mut git_args = args.to_vec();
    git_args.push("--color=always");
    let git_output = repo.run_command(&git_args)?;

    // Try to pipe through configured renderer
    // This is synchronous (no threading) to avoid concurrency issues
    let result = match get_pager_config() {
        Some(pager_cmd) => {
            log::debug!("Invoking renderer: {}", pager_cmd);

            // SECURITY NOTE: Using sh -c to invoke renderer inherits git's security model.
            // Git itself uses sh -c for pagers (for shell features like pipes, aliases, etc.)
            // Users who can control GIT_PAGER/PAGER can already execute arbitrary commands
            // via normal git operations, so this doesn't introduce new attack surface.
            // The renderer command comes from trusted sources (user's own env vars and git config).

            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg(pager_cmd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());

            // Set environment variables to disable interactive paging features.
            // This works generically across all renderers without needing tool-specific flags.
            // Environment variable precedence (tools check in this order):
            // - Delta: DELTA_PAGER → BAT_PAGER → PAGER
            // - Bat: BAT_PAGER → PAGER
            // - Less/others: PAGER
            cmd.env("PAGER", "cat") // Generic fallback for all tools
                .env("DELTA_PAGER", "cat") // Delta-specific (highest priority for delta)
                .env("BAT_PAGER", ""); // Bat-specific (empty string disables paging)

            // Spawn and immediately wait - synchronous execution
            match cmd.spawn() {
                Ok(mut child) => {
                    // Write git output to renderer's stdin and explicitly close it
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(git_output.as_bytes());
                        // Explicitly drop stdin to close the pipe
                        // This signals EOF to the renderer so it knows to process and exit
                        drop(stdin);
                    }

                    // Wait for renderer to complete (synchronous)
                    // Note: If renderer hangs indefinitely, this will block. However:
                    // - We only invoke this after verifying non-empty stat output
                    // - We explicitly close stdin (drop above) to signal EOF
                    // - Renderers like delta/bat are designed to process and exit quickly
                    // - This is same behavior as git's pager invocation
                    match child.wait_with_output() {
                        Ok(output) if output.status.success() => {
                            log::debug!("Renderer succeeded, output len={}", output.stdout.len());
                            // Success - return renderer output
                            String::from_utf8(output.stdout).unwrap_or(git_output.clone())
                        }
                        Ok(output) => {
                            log::debug!(
                                "Renderer failed with status={:?}, falling back",
                                output.status
                            );
                            // Renderer failed - fall back to plain colored output
                            git_output.clone()
                        }
                        Err(e) => {
                            log::debug!("Renderer wait error: {}, falling back", e);
                            // Wait failed - fall back to plain colored output
                            // Note: child process is consumed by wait_with_output(),
                            // so we can't kill it from here. The OS will clean it up
                            // when the parent process exits.
                            git_output.clone()
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Renderer spawn failed: {}, falling back", e);
                    // Spawn failed - fall back to plain colored output
                    git_output.clone()
                }
            }
        }
        None => {
            log::debug!("No renderer configured, using git output directly");
            // No renderer configured - return git output directly
            git_output
        }
    };

    Ok(result)
}

/// Wrapper to implement SkimItem for ListItem
struct WorktreeSkimItem {
    display_text: String,
    branch_name: String,
    item: Arc<ListItem>,
}

impl SkimItem for WorktreeSkimItem {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.display_text)
    }

    fn output(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.branch_name)
    }

    fn preview(&self, _context: PreviewContext<'_>) -> ItemPreview {
        let mode = PreviewMode::read_from_state();
        let preview_text = match mode {
            PreviewMode::WorkingTree => self.render_working_tree_preview(),
            PreviewMode::History => self.render_history_preview(),
            PreviewMode::BranchDiff => self.render_branch_diff_preview(),
        };

        ItemPreview::AnsiText(preview_text)
    }
}

impl WorktreeSkimItem {
    /// Render Mode 1: Working tree preview (uncommitted changes vs HEAD)
    /// Matches `wt list` "HEAD±" column
    fn render_working_tree_preview(&self) -> String {
        let mut output = String::new();
        let repo = Repository::current();

        let Some(wt_info) = self.item.worktree_info() else {
            output.push_str("No worktree (branch only)\n");
            return output;
        };

        let path_str = wt_info.worktree.path.display().to_string();

        // Show working tree changes as --stat (uncommitted changes)
        // Check without color first to see if there's any content
        if let Ok(diff_stat) = repo.run_command(&["-C", &path_str, "diff", "HEAD", "--stat"])
            && !diff_stat.trim().is_empty()
        {
            output.push_str(&diff_stat);
            output.push('\n');
            output.push('\n'); // Visual separator

            // Show full diff below the stat summary (with renderer if configured)
            if let Ok(diff) = run_diff_with_pager(&repo, &["-C", &path_str, "diff", "HEAD"]) {
                output.push_str(&diff);
            }
        } else {
            output.push_str("No uncommitted changes\n");
        }

        output
    }

    /// Render Mode 3: Branch diff preview (line diffs in commits ahead of main)
    /// Matches `wt list` "main…± (--full)" column
    fn render_branch_diff_preview(&self) -> String {
        let mut output = String::new();
        let repo = Repository::current();
        let counts = self.item.counts();

        if counts.ahead > 0 {
            let head = self.item.head();
            let merge_base = format!("main...{}", head);
            // Check without color first to see if there's any content
            if let Ok(diff_stat) = repo.run_command(&["diff", "--stat", &merge_base])
                && !diff_stat.trim().is_empty()
            {
                output.push_str(&diff_stat);
                output.push('\n');
                output.push('\n'); // Visual separator

                // Show full diff below the stat summary (with renderer if configured)
                if let Ok(diff) = run_diff_with_pager(&repo, &["diff", &merge_base]) {
                    output.push_str(&diff);
                }
            } else {
                output.push_str("No changes vs main\n");
            }
        } else {
            output.push_str("No commits ahead of main\n");
        }

        output
    }

    /// Render Mode 2: History preview
    fn render_history_preview(&self) -> String {
        const HISTORY_LIMIT: &str = "10";

        let mut output = String::new();
        let repo = Repository::current();
        let head = self.item.head();

        // Get merge-base with main
        //
        // Note on error handling: This code runs in an interactive preview pane that updates
        // on every keystroke. We intentionally use silent fallbacks rather than propagating
        // errors to avoid disruptive error messages during navigation. The preview is
        // supplementary - users can still select worktrees even if preview fails.
        //
        // Alternative: Check specific conditions (main branch exists, valid HEAD, etc.) before
        // running git commands. This would provide better diagnostics but adds latency to
        // every preview render. Trade-off: simplicity + speed vs. detailed error messages.
        let Ok(merge_base_output) = repo.run_command(&["merge-base", "main", head]) else {
            output.push_str("No commits\n");
            return output;
        };

        let merge_base = merge_base_output.trim();

        let branch = self.item.branch_name();
        let is_main = branch == "main" || branch == "master";

        if is_main {
            // Viewing main itself - show history without dimming
            if let Ok(log_output) = repo.run_command(&[
                "log",
                "--graph",
                "--decorate",
                "--oneline",
                "--color=always",
                "-n",
                HISTORY_LIMIT,
                head,
            ]) {
                output.push_str(&log_output);
            }
        } else {
            // Not on main - show bright commits not on main, dimmed commits on main

            // Part 1: Bright commits (merge-base..HEAD)
            let range = format!("{}..{}", merge_base, head);
            if let Ok(log_output) =
                repo.run_command(&["log", "--graph", "--oneline", "--color=always", &range])
            {
                output.push_str(&log_output);
            }

            // Part 2: Dimmed commits on main (history before merge-base)
            if let Ok(log_output) = repo.run_command(&[
                "log",
                "--graph",
                "--oneline",
                "--format=%C(dim)%h %s%C(reset)",
                "--color=always",
                "-n",
                HISTORY_LIMIT,
                merge_base,
            ]) {
                output.push_str(&log_output);
            }
        }

        output
    }
}

pub fn handle_select() -> Result<(), GitError> {
    let repo = Repository::current();

    // Initialize preview mode state file (default to WorkingTree)
    let state_path = PreviewMode::state_path();
    if !state_path.exists() {
        let _ = fs::write(&state_path, "1");
    }

    // Gather list data using existing logic
    let Some(list_data) = gather_list_data(&repo, false, false, false)? else {
        return Ok(());
    };

    // Calculate max branch name length for alignment
    let max_branch_len = list_data
        .items
        .iter()
        .map(|item| item.branch_name().len())
        .max()
        .unwrap_or(20);

    // Convert to skim items - store full ListItem for preview rendering
    let items: Vec<Arc<dyn SkimItem>> = list_data
        .items
        .into_iter()
        .map(|item| {
            let branch_name = item.branch_name().to_string();
            let commit_msg = item
                .commit_details()
                .commit_message
                .lines()
                .next()
                .unwrap_or("");

            // Build display text with aligned columns
            let mut display_text = format!("{:<width$}", branch_name, width = max_branch_len);

            // Add status symbols for worktrees (fixed width)
            let status = if let Some(wt_info) = item.worktree_info() {
                format!("{:^8}", wt_info.status_symbols.render())
            } else {
                "        ".to_string()
            };
            display_text.push_str(&status);

            // Add commit message
            display_text.push_str("  ");
            display_text.push_str(commit_msg);

            Arc::new(WorktreeSkimItem {
                display_text,
                branch_name,
                item: Arc::new(item),
            }) as Arc<dyn SkimItem>
        })
        .collect();

    // Get state path for key bindings
    let state_path_str = state_path.display().to_string();

    // Configure skim options with Rust-based preview and mode switching keybindings
    let options = SkimOptionsBuilder::default()
        .height("50%".to_string())
        .multi(false)
        .preview(Some("".to_string())) // Enable preview (empty string means use SkimItem::preview())
        .preview_window("right:50%".to_string())
        .color(Some(
            "fg:-1,bg:-1,matched:108,current:-1,current_bg:254,current_match:108".to_string(),
        ))
        .bind(vec![
            // Mode switching
            format!(
                "1:execute-silent(echo 1 > {})+refresh-preview",
                state_path_str
            ),
            format!(
                "2:execute-silent(echo 2 > {})+refresh-preview",
                state_path_str
            ),
            format!(
                "3:execute-silent(echo 3 > {})+refresh-preview",
                state_path_str
            ),
            // Preview scrolling
            "ctrl-u:preview-page-up".to_string(),
            "ctrl-d:preview-page-down".to_string(),
        ])
        .header(Some(
            "1: working | 2: history | 3: diff | ctrl-u/d: scroll | ctrl-/: toggle".to_string(),
        ))
        .build()
        .map_err(|e| GitError::CommandFailed(format!("Failed to build skim options: {}", e)))?;

    // Create item receiver
    let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();
    for item in items {
        tx.send(item)
            .map_err(|e| GitError::CommandFailed(format!("Failed to send item to skim: {}", e)))?;
    }
    drop(tx);

    // Run skim
    let output = Skim::run_with(&options, Some(rx));

    // Clean up state file
    let _ = fs::remove_file(&state_path);

    // Handle selection
    if let Some(out) = output
        && !out.is_abort
        && let Some(selected) = out.selected_items.first()
    {
        // Get branch name or worktree path from selected item
        // (output() returns the worktree path for existing worktrees, branch name otherwise)
        let identifier = selected.output().to_string();

        // Load config
        let config = WorktrunkConfig::load().git_context("Failed to load config")?;

        // Switch to the selected worktree
        // handle_switch can handle both branch names and worktree paths
        let (result, resolved_branch) =
            handle_switch(&identifier, false, None, false, false, &config)?;

        // Show success message (show shell integration hint if not configured)
        handle_switch_output(&result, &resolved_branch, false)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_mode_from_u8() {
        assert_eq!(PreviewMode::from_u8(1), PreviewMode::WorkingTree);
        assert_eq!(PreviewMode::from_u8(2), PreviewMode::History);
        assert_eq!(PreviewMode::from_u8(3), PreviewMode::BranchDiff);
        // Invalid values default to WorkingTree
        assert_eq!(PreviewMode::from_u8(0), PreviewMode::WorkingTree);
        assert_eq!(PreviewMode::from_u8(99), PreviewMode::WorkingTree);
    }

    #[test]
    fn test_preview_mode_state_file_read_default() {
        // When state file doesn't exist or is invalid, default to WorkingTree
        let state_path = PreviewMode::state_path();
        // Clean up any existing state
        let _ = fs::remove_file(&state_path);

        assert_eq!(PreviewMode::read_from_state(), PreviewMode::WorkingTree);
    }

    #[test]
    fn test_preview_mode_state_file_roundtrip() {
        // Use a unique test file to avoid conflicts with concurrent tests
        let test_state_path =
            std::env::temp_dir().join(format!("wt-select-mode-test-{}", std::process::id()));

        // Write mode 1 (WorkingTree)
        fs::write(&test_state_path, "1").unwrap();
        let mode = fs::read_to_string(&test_state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(PreviewMode::from_u8)
            .unwrap_or(PreviewMode::WorkingTree);
        assert_eq!(mode, PreviewMode::WorkingTree);

        // Write mode 2 (History)
        fs::write(&test_state_path, "2").unwrap();
        let mode = fs::read_to_string(&test_state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(PreviewMode::from_u8)
            .unwrap_or(PreviewMode::WorkingTree);
        assert_eq!(mode, PreviewMode::History);

        // Write mode 3 (BranchDiff)
        fs::write(&test_state_path, "3").unwrap();
        let mode = fs::read_to_string(&test_state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(PreviewMode::from_u8)
            .unwrap_or(PreviewMode::WorkingTree);
        assert_eq!(mode, PreviewMode::BranchDiff);

        // Cleanup
        let _ = fs::remove_file(&test_state_path);
    }
}

//! Global output context using thread-local storage
//!
//! This provides a logging-like API where you configure output mode once
//! at program start, then use it anywhere without passing parameters.
//!
//! # Implementation
//!
//! Uses `thread_local!` to store per-thread output state:
//! - Each thread gets its own `OUTPUT_CONTEXT`
//! - `RefCell<T>` enables interior mutability (runtime borrow checking)
//! - Enum dispatch avoids trait object overhead (static dispatch)
//!
//! # Trade-offs
//!
//! - ✅ Zero parameter threading - call from anywhere
//! - ✅ Single initialization point - set once in main()
//! - ✅ Fast access - thread-local is just a pointer lookup
//! - ⚠️ Per-thread state - not an issue for single-threaded CLI
//! - ⚠️ Runtime borrow checks - acceptable for this access pattern

use super::directive::DirectiveOutput;
use super::interactive::InteractiveOutput;
use std::cell::RefCell;
use std::io;
use std::path::Path;

/// Output mode selection
#[derive(Debug, Clone, Copy)]
pub enum OutputMode {
    Interactive,
    Directive,
}

/// Output handler - enum dispatch instead of trait object
enum OutputHandler {
    Interactive(InteractiveOutput),
    Directive(DirectiveOutput),
}

thread_local! {
    static OUTPUT_CONTEXT: RefCell<OutputHandler> = RefCell::new(
        OutputHandler::Interactive(InteractiveOutput::new())
    );
}

/// Initialize the global output context
///
/// Call this once at program startup to set the output mode.
pub fn initialize(mode: OutputMode) {
    let handler = match mode {
        OutputMode::Interactive => OutputHandler::Interactive(InteractiveOutput::new()),
        OutputMode::Directive => OutputHandler::Directive(DirectiveOutput::new()),
    };

    OUTPUT_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = handler;
    });
}

/// Emit a success message
pub fn success(message: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let msg = message.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.success(msg),
            OutputHandler::Directive(d) => d.success(msg),
        }
    })
}

/// Emit a progress message
///
/// Progress messages are intermediate status updates like "🔄 Cleaning up worktree..."
/// They are shown to users in both modes (users need to see what's happening).
pub fn progress(message: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let msg = message.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.progress(msg),
            OutputHandler::Directive(d) => d.progress(msg),
        }
    })
}

/// Display a hint message
///
/// Hints are suggestions for users, like "Backup created @ \<sha\>" or "Using fallback commit message"
pub fn hint(message: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let msg = message.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.hint(msg),
            OutputHandler::Directive(d) => d.hint(msg),
        }
    })
}

/// Display a shell integration hint (suppressed in directive mode)
///
/// Shell integration hints like "To enable automatic cd, run: wt config shell" are only
/// shown in interactive mode since directive mode users already have shell integration
pub fn shell_integration_hint(message: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let msg = message.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.shell_integration_hint(msg),
            OutputHandler::Directive(d) => d.shell_integration_hint(msg),
        }
    })
}

/// Emit an info message
///
/// Info messages are neutral status updates like "⚪ No changes detected"
/// They use INFO_EMOJI (⚪) and dimmed styling.
pub fn info(message: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let msg = message.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.info(msg),
            OutputHandler::Directive(d) => d.info(msg),
        }
    })
}

/// Emit a warning message
///
/// Warning messages are non-blocking issues like "🟡 Uncommitted changes detected"
/// They use WARNING_EMOJI (🟡) and warning styling.
pub fn warning(message: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let msg = message.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.warning(msg),
            OutputHandler::Directive(d) => d.warning(msg),
        }
    })
}

/// Emit an error message
///
/// Error messages are critical failures like "❌ Cannot remove main worktree"
/// The message is already formatted (includes ERROR_EMOJI from GitError::Display).
///
/// In interactive mode: goes to stdout (with other worktrunk output)
/// In directive mode: goes to stderr (with other user-facing messages)
pub fn error(message: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let msg = message.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.error(msg),
            OutputHandler::Directive(d) => d.error(msg),
        }
    })
}

/// Insert a blank line between UI output and worktrunk messages
#[cfg(unix)]
pub fn blank_line() -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| match &mut *ctx.borrow_mut() {
        OutputHandler::Interactive(i) => i.blank_line(),
        OutputHandler::Directive(d) => d.blank_line(),
    })
}

/// Emit gutter-formatted content
///
/// Gutter content has its own visual structure (column 0 gutter + content),
/// so no additional emoji is added. Use with `format_with_gutter()` or `format_bash_with_gutter()`.
pub fn gutter(content: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let c = content.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.gutter(c),
            OutputHandler::Directive(d) => d.gutter(c),
        }
    })
}

/// Emit raw output without emoji decoration
///
/// Used for structured data like JSON. Goes to stdout in interactive mode,
/// stderr in directive mode (where stdout is reserved for directives).
///
/// Example:
/// ```rust,ignore
/// output::raw(json_string)?;
/// ```
pub fn raw(content: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let c = content.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.raw(c),
            OutputHandler::Directive(d) => d.raw(c),
        }
    })
}

/// Emit raw terminal output to stderr
///
/// Used for table output that should appear on the same stream as progress bars.
/// Goes to stderr in both interactive and directive modes.
///
/// TODO: This split between raw() and raw_terminal() is messy. Consider unifying
/// the output system to have a clearer separation between structured data (JSON)
/// and terminal UI (tables, progress bars).
///
/// Example:
/// ```rust,ignore
/// output::raw_terminal(layout.format_header_line())?;
/// for item in items {
///     output::raw_terminal(layout.format_item_line(item))?;
/// }
/// ```
pub fn raw_terminal(content: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let c = content.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.raw_terminal(c),
            OutputHandler::Directive(d) => d.raw_terminal(c),
        }
    })
}

/// Request directory change (for shell integration)
pub fn change_directory(path: impl AsRef<Path>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let p = path.as_ref();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.change_directory(p),
            OutputHandler::Directive(d) => d.change_directory(p),
        }
    })
}

/// Request command execution
pub fn execute(command: impl Into<String>) -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| {
        let cmd = command.into();
        match &mut *ctx.borrow_mut() {
            OutputHandler::Interactive(i) => i.execute(cmd),
            OutputHandler::Directive(d) => d.execute(cmd),
        }
    })
}

/// Flush any buffered output
pub fn flush() -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| match &mut *ctx.borrow_mut() {
        OutputHandler::Interactive(i) => i.flush(),
        OutputHandler::Directive(d) => d.flush(),
    })
}

/// Flush streams before showing stderr prompt
///
/// This prevents stream interleaving. Interactive prompts write to stderr, so we must
/// ensure all previous output is flushed first:
/// - In directive mode: Flushes both stdout (directives) and stderr (messages)
/// - In interactive mode: Flushes both stdout and stderr
///
/// Note: With stderr separation (messages on stderr in directive mode), prompts
/// naturally appear after messages without needing NUL terminators for synchronization.
pub fn flush_for_stderr_prompt() -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| match &mut *ctx.borrow_mut() {
        OutputHandler::Interactive(i) => i.flush_for_stderr_prompt(),
        OutputHandler::Directive(d) => d.flush_for_stderr_prompt(),
    })
}

/// Terminate command output
///
/// In directive mode, writes a NUL terminator to separate command output from
/// subsequent directives. In interactive mode, this is a no-op.
pub fn terminate_output() -> io::Result<()> {
    OUTPUT_CONTEXT.with(|ctx| match &mut *ctx.borrow_mut() {
        OutputHandler::Interactive(i) => i.terminate_output(),
        OutputHandler::Directive(d) => d.terminate_output(),
    })
}

/// Format a switch success message (identical across modes)
///
/// Both modes now report `"at {path}"` so users see the same wording whether
/// they invoke worktrunk directly or through the shell wrapper.
pub fn format_switch_success(
    branch: &str,
    path: &Path,
    created_branch: bool,
    base_branch: Option<&str>,
) -> String {
    OUTPUT_CONTEXT.with(|ctx| match &*ctx.borrow() {
        OutputHandler::Interactive(i) => {
            i.format_switch_success(branch, path, created_branch, base_branch)
        }
        OutputHandler::Directive(d) => {
            d.format_switch_success(branch, path, created_branch, base_branch)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_switching() {
        // Default is interactive
        initialize(OutputMode::Interactive);
        // Just verify initialize doesn't panic

        // Switch to directive
        initialize(OutputMode::Directive);
        // Just verify initialize doesn't panic
    }
}

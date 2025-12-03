//! Style constants and emojis for terminal output
//!
//! # Styling with color-print
//!
//! Use `cformat!` with HTML-like tags for all user-facing messages:
//!
//! ```rust,ignore
//! use color_print::cformat;
//!
//! // Simple styling
//! cformat!("<green>Success message</>")
//!
//! // Nested styles - bold inherits green
//! cformat!("<green>Removed branch <bold>{branch}</> successfully</>")
//!
//! // Semantic mapping:
//! // - Errors: <red>...</>
//! // - Warnings: <yellow>...</>
//! // - Hints: <dim>...</>
//! // - Progress: <cyan>...</>
//! // - Success: <green>...</>
//! // - Secondary: <bright-black>...</>
//! ```
//!
//! # anstyle constants
//!
//! A few `Style` constants remain for programmatic use with `StyledLine` and
//! table rendering where computed styles are needed at runtime.

use anstyle::{AnsiColor, Color, Style};

// ============================================================================
// Programmatic Style Constants (for StyledLine, tables, computed styles)
// ============================================================================

/// Addition style for diffs (green) - used in table rendering
pub const ADDITION: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));

/// Deletion style for diffs (red) - used in table rendering
pub const DELETION: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));

/// Gutter style for quoted content (commands, config, error details)
///
/// We wanted the dimmest/most subtle background that works on both dark and light
/// terminals. BrightWhite was the best we could find among basic ANSI colors, but
/// we're open to better ideas. Options considered:
/// - Black/BrightBlack: too dark on light terminals
/// - Reverse video: just flips which terminal looks good
/// - 256-color grays: better but not universally supported
/// - No background: loses the visual separation we want
pub const GUTTER: Style = Style::new().bg_color(Some(Color::Ansi(AnsiColor::BrightWhite)));

// ============================================================================
// Message Emojis
// ============================================================================

/// Progress emoji: `cformat!("{PROGRESS_EMOJI} <cyan>message</>")`
pub const PROGRESS_EMOJI: &str = "🔄";

/// Success emoji: `cformat!("{SUCCESS_EMOJI} <green>message</>")`
pub const SUCCESS_EMOJI: &str = "✅";

/// Error emoji: `cformat!("{ERROR_EMOJI} <red>message</>")`
pub const ERROR_EMOJI: &str = "❌";

/// Warning emoji: `cformat!("{WARNING_EMOJI} <yellow>message</>")`
pub const WARNING_EMOJI: &str = "🟡";

/// Hint emoji: `cformat!("{HINT_EMOJI} <dim>message</>")`
pub const HINT_EMOJI: &str = "💡";

/// Info emoji - use for neutral status (primary status NOT dimmed, metadata may be dimmed)
/// Primary status: `output::info("All commands already approved")?;`
/// Metadata: `cformat!("{INFO_EMOJI} <dim>Showing 5 worktrees...</>")`
pub const INFO_EMOJI: &str = "⚪";

/// Prompt emoji - use for questions requiring user input
/// `eprint!("{PROMPT_EMOJI} Proceed? [y/N] ")`
pub const PROMPT_EMOJI: &str = "❓";

//! Markdown rendering for CLI help text using termimad.

use termimad::{MadSkin, crossterm::style::Color};

/// Render markdown in help text to ANSI with minimal styling (green headers only)
pub fn render_markdown_in_help(help: &str) -> String {
    let mut skin = MadSkin::no_style();
    skin.headers[0].set_fg(Color::Green);
    skin.headers[1].set_fg(Color::Green);

    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80);

    let rendered = format!("{}", skin.text(help, Some(width)));

    // Color status symbols to match their descriptions
    colorize_status_symbols(&rendered)
}

/// Add colors to status symbols in help text (matching wt list output colors)
fn colorize_status_symbols(text: &str) -> String {
    use anstyle::{AnsiColor, Color as AnsiStyleColor, Style};

    // Define semantic styles matching src/commands/list/model.rs StatusSymbols::render_with_mask
    let error = Style::new().fg_color(Some(AnsiStyleColor::Ansi(AnsiColor::Red)));
    let warning = Style::new().fg_color(Some(AnsiStyleColor::Ansi(AnsiColor::Yellow)));
    let hint = Style::new().dimmed();
    let success = Style::new().fg_color(Some(AnsiStyleColor::Ansi(AnsiColor::Green)));
    let progress = Style::new().fg_color(Some(AnsiStyleColor::Ansi(AnsiColor::Blue)));
    let disabled = Style::new().fg_color(Some(AnsiStyleColor::Ansi(AnsiColor::BrightBlack)));
    let working_tree = Style::new().fg_color(Some(AnsiStyleColor::Ansi(AnsiColor::Cyan)));

    text
        // CI status circles
        .replace("● passed", &format!("{success}●{success:#} passed"))
        .replace("● running", &format!("{progress}●{progress:#} running"))
        .replace("● failed", &format!("{error}●{error:#} failed"))
        .replace("● conflicts", &format!("{warning}●{warning:#} conflicts"))
        .replace("● no-ci", &format!("{disabled}●{disabled:#} no-ci"))
        // Conflicts: ✖ is ERROR (red), ⚠ is WARNING (yellow)
        .replace(
            "✖ Merge conflicts",
            &format!("{error}✖{error:#} Merge conflicts"),
        )
        .replace(
            "⚠ Would conflict",
            &format!("{warning}⚠{warning:#} Would conflict"),
        )
        // Git operations: WARNING (yellow)
        .replace("↻ Rebase", &format!("{warning}↻{warning:#} Rebase"))
        .replace("⋈ Merge", &format!("{warning}⋈{warning:#} Merge"))
        // Worktree attributes: WARNING (yellow)
        .replace("⊠ Locked", &format!("{warning}⊠{warning:#} Locked"))
        .replace("⚠ Prunable", &format!("{warning}⚠{warning:#} Prunable"))
        // Branch state: HINT (dimmed)
        .replace(
            "≡ Working tree matches",
            &format!("{hint}≡{hint:#} Working tree matches"),
        )
        .replace("∅ No commits", &format!("{hint}∅{hint:#} No commits"))
        .replace(
            "· Branch without",
            &format!("{hint}·{hint:#} Branch without"),
        )
        // Main/upstream divergence: NO COLOR (plain text in actual output)
        // ↑, ↓, ↕, ⇡, ⇣, ⇅ remain uncolored
        // Working tree changes: CYAN
        .replace(
            "? Untracked",
            &format!("{working_tree}?{working_tree:#} Untracked"),
        )
        .replace(
            "! Modified",
            &format!("{working_tree}!{working_tree:#} Modified"),
        )
        .replace(
            "+ Staged",
            &format!("{working_tree}+{working_tree:#} Staged"),
        )
        .replace(
            "» Renamed",
            &format!("{working_tree}»{working_tree:#} Renamed"),
        )
        .replace(
            "✘ Deleted",
            &format!("{working_tree}✘{working_tree:#} Deleted"),
        )
}

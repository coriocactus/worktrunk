use anstyle::Style;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthStr;

/// A piece of text with an optional style
#[derive(Clone, Debug)]
pub struct StyledString {
    pub text: String,
    pub style: Option<Style>,
}

impl StyledString {
    pub fn new(text: impl Into<String>, style: Option<Style>) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn raw(text: impl Into<String>) -> Self {
        Self::new(text, None)
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self::new(text, Some(style))
    }

    /// Returns the visual width (unicode-aware, no ANSI codes)
    pub fn width(&self) -> usize {
        self.text.width()
    }

    /// Renders to a string with ANSI escape codes
    pub fn render(&self) -> String {
        if let Some(style) = &self.style {
            format!("{}{}{}", style.render(), self.text, style.render_reset())
        } else {
            self.text.clone()
        }
    }
}

/// A line composed of multiple styled strings
#[derive(Clone, Debug, Default)]
pub struct StyledLine {
    pub segments: Vec<StyledString>,
}

impl StyledLine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a raw (unstyled) segment
    pub fn push_raw(&mut self, text: impl Into<String>) {
        self.segments.push(StyledString::raw(text));
    }

    /// Add a styled segment
    pub fn push_styled(&mut self, text: impl Into<String>, style: Style) {
        self.segments.push(StyledString::styled(text, style));
    }

    /// Add a segment (StyledString)
    pub fn push(&mut self, segment: StyledString) {
        self.segments.push(segment);
    }

    /// Pad with spaces to reach a specific width
    pub fn pad_to(&mut self, target_width: usize) {
        let current_width = self.width();
        if current_width < target_width {
            self.push_raw(" ".repeat(target_width - current_width));
        }
    }

    /// Returns the total visual width
    pub fn width(&self) -> usize {
        self.segments.iter().map(|s| s.width()).sum()
    }

    /// Renders the entire line with ANSI escape codes
    pub fn render(&self) -> String {
        self.segments.iter().map(|s| s.render()).collect()
    }
}

pub fn format_relative_time(timestamp: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let seconds_ago = now - timestamp;

    if seconds_ago < 0 {
        return "in the future".to_string();
    }

    let minutes = seconds_ago / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let weeks = days / 7;
    let months = days / 30;
    let years = days / 365;

    if years > 0 {
        format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
    } else if months > 0 {
        format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
    } else if weeks > 0 {
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else if days > 0 {
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    } else if hours > 0 {
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if minutes > 0 {
        format!(
            "{} minute{} ago",
            minutes,
            if minutes == 1 { "" } else { "s" }
        )
    } else {
        "just now".to_string()
    }
}

/// Find the common prefix among all paths
pub fn find_common_prefix(paths: &[PathBuf]) -> PathBuf {
    if paths.is_empty() {
        return PathBuf::new();
    }

    let first = &paths[0];
    let mut prefix = PathBuf::new();

    for component in first.components() {
        let candidate = prefix.join(component);
        if paths.iter().all(|p| p.starts_with(&candidate)) {
            prefix = candidate;
        } else {
            break;
        }
    }

    prefix
}

/// Shorten a path relative to a common prefix
pub fn shorten_path(path: &Path, prefix: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(prefix) {
        if relative.as_os_str().is_empty() {
            ".".to_string()
        } else {
            format!("./{}", relative.display())
        }
    } else {
        path.display().to_string()
    }
}

/// Truncate text at word boundary with ellipsis, respecting terminal width
pub fn truncate_at_word_boundary(text: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthStr;

    if text.width() <= max_width {
        return text.to_string();
    }

    // Build up string until we hit the width limit (accounting for "..." = 3 width)
    let target_width = max_width.saturating_sub(3);
    let mut current_width = 0;
    let mut last_space_idx = None;
    let mut last_idx = 0;

    for (idx, ch) in text.char_indices() {
        let char_width = UnicodeWidthStr::width(ch.to_string().as_str());
        if current_width + char_width > target_width {
            break;
        }
        if ch.is_whitespace() {
            last_space_idx = Some(idx);
        }
        current_width += char_width;
        last_idx = idx + ch.len_utf8();
    }

    // Use last space if found, otherwise truncate at last character that fits
    let truncate_at = last_space_idx.unwrap_or(last_idx);
    format!("{}...", &text[..truncate_at].trim())
}

/// Get terminal width, defaulting to 80 if detection fails
pub fn get_terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(terminal_size::Width(w), _)| w as usize)
        .unwrap_or(80)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_styled_string_width() {
        // ASCII strings
        let s = StyledString::raw("hello");
        assert_eq!(s.width(), 5);

        // Unicode arrows
        let s = StyledString::raw("↑3 ↓2");
        assert_eq!(
            s.width(),
            5,
            "↑3 ↓2 should have width 5, not {}",
            s.text.len()
        );

        // Mixed Unicode
        let s = StyledString::raw("日本語");
        assert_eq!(s.width(), 6); // CJK characters are typically width 2

        // Emoji
        let s = StyledString::raw("🎉");
        assert_eq!(s.width(), 2); // Emoji are typically width 2
    }

    #[test]
    fn test_styled_line_width() {
        let mut line = StyledLine::new();
        line.push_raw("Branch");
        line.push_raw("  ");
        line.push_raw("↑3 ↓2");

        // "Branch" (6) + "  " (2) + "↑3 ↓2" (5) = 13
        assert_eq!(line.width(), 13, "Line width should be 13");
    }

    #[test]
    fn test_styled_line_padding() {
        let mut line = StyledLine::new();
        line.push_raw("test");
        assert_eq!(line.width(), 4);

        line.pad_to(10);
        assert_eq!(line.width(), 10, "After padding to 10, width should be 10");

        // Padding when already at target should not change width
        line.pad_to(10);
        assert_eq!(line.width(), 10, "Padding again should not change width");
    }

    #[test]
    fn test_sparse_column_padding() {
        // Build simplified lines to test sparse column padding
        let mut line1 = StyledLine::new();
        line1.push_raw(format!("{:8}", "branch-a"));
        line1.push_raw("  ");
        // Has ahead/behind
        line1.push_raw(format!("{:5}", "↑3 ↓2"));
        line1.push_raw("  ");

        let mut line2 = StyledLine::new();
        line2.push_raw(format!("{:8}", "branch-b"));
        line2.push_raw("  ");
        // No ahead/behind, should pad with spaces
        line2.push_raw(" ".repeat(5));
        line2.push_raw("  ");

        // Both lines should have same width up to this point
        assert_eq!(
            line1.width(),
            line2.width(),
            "Rows with and without sparse column data should have same width"
        );
    }
}

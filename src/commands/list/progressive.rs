/// Rendering mode for list command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Buffered: collect all data, then render (traditional)
    Buffered,
    /// Progressive: show rows immediately, update as data arrives
    Progressive,
}

impl RenderMode {
    /// Determine rendering mode based on CLI flags and TTY status
    ///
    /// # Arguments
    ///
    /// * `progressive` - Rendering mode (Some(true) = --progressive, Some(false) = --no-progressive, None = auto)
    ///
    /// Table output always goes to stderr (via output::table()), so we check stderr's TTY status.
    pub fn detect(progressive: Option<bool>) -> Self {
        // Priority 1: Explicit CLI flag
        match progressive {
            Some(true) => return RenderMode::Progressive,
            Some(false) => return RenderMode::Buffered,
            None => {} // Fall through to auto-detection
        }

        // Priority 2: Auto-detect based on stderr TTY (table output always goes to stderr)
        use std::io::IsTerminal;
        if std::io::stderr().is_terminal() {
            // TODO: Check for pager in environment
            RenderMode::Progressive
        } else {
            RenderMode::Buffered
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_mode_detect_explicit_flags() {
        // --progressive (Some(true)) should force progressive mode
        assert_eq!(RenderMode::detect(Some(true)), RenderMode::Progressive);

        // --no-progressive (Some(false)) should force buffered mode
        assert_eq!(RenderMode::detect(Some(false)), RenderMode::Buffered);

        // None should auto-detect (tested via TTY checks in runtime)
    }
}

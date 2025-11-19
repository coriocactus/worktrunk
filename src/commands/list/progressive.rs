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
    /// * `progressive_flag` - Explicit --progressive flag
    /// * `no_progressive_flag` - Explicit --no-progressive flag
    /// * `directive_mode` - True if in directive mode (--internal), affects which stream to check
    ///
    /// In directive mode, table output goes to stderr, so we check stderr's TTY status.
    /// In interactive mode, table output goes to stdout, so we check stdout's TTY status.
    pub fn detect(progressive_flag: bool, no_progressive_flag: bool, directive_mode: bool) -> Self {
        // Priority 1: Explicit CLI flags
        if progressive_flag {
            return RenderMode::Progressive;
        }
        if no_progressive_flag {
            return RenderMode::Buffered;
        }

        // Priority 2: Auto-detect based on TTY
        // Check the appropriate stream based on output mode:
        // - Directive mode: check stderr (where table output goes via output::raw())
        // - Interactive mode: check stdout (where table output goes)
        use std::io::IsTerminal;
        let is_tty = if directive_mode {
            std::io::stderr().is_terminal()
        } else {
            std::io::stdout().is_terminal()
        };

        if is_tty {
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
        // Explicit flags should work regardless of directive mode
        assert_eq!(
            RenderMode::detect(true, false, false),
            RenderMode::Progressive
        );
        assert_eq!(RenderMode::detect(false, true, false), RenderMode::Buffered);
        assert_eq!(
            RenderMode::detect(true, false, true),
            RenderMode::Progressive
        );
        assert_eq!(RenderMode::detect(false, true, true), RenderMode::Buffered);
    }
}

//! Column layout and priority allocation for the list command.
//!
//! # Status Column Structure
//!
//! The Status column uses a unified position-based grid system for all status
//! indicators including user-defined status:
//!
//! ```text
//! Status Column = Dynamic Grid [Position 0a...Position 4]
//! ```
//!
//! ## Unified Position Grid
//!
//! All status indicators use position-based alignment with selective rendering:
//! - Position 0a: Conflicts (=)
//! - Position 0b: Branch state (≡, ∅)
//! - Position 0c: Git operation (↻, ⋈)
//! - Position 0d: Worktree attributes (⊠, ⚠)
//! - Position 1: Main divergence (↑, ↓, ↕)
//! - Position 2: Upstream divergence (⇡, ⇣, ⇅)
//! - Position 3: Working tree (?, !, +, », ✘)
//! - Position 4: User status (custom labels, emoji)
//!
//! Only positions used by at least one row are included (position mask):
//! - Within those positions, symbols align vertically for scannability
//! - Empty positions render as single space for grid alignment
//! - No leading spaces before the first symbol
//!
//! Example with positions 0b, 3, and 4 used:
//! ```text
//! Row 1: "≡   🤖"   (0b=≡, 3=space, 4=🤖)
//! Row 2: "≡?!   "   (0b=≡, 3=?!, 4=space)
//! Row 3: "  💬"     (0b=space, 3=space, 4=💬)
//! ```
//!
//! ## Width Calculation
//!
//! ```text
//! status_width = max(rendered_width_across_all_items)
//! ```
//!
//! The width is calculated by rendering each item's status with the position
//! mask and taking the maximum width.
//!
//! ## Why This Design?
//!
//! **Single canonical system:**
//! - One alignment mechanism for all status indicators
//! - User status treated consistently with git symbols
//!
//! **Eliminates wasted space:**
//! - Position mask removes columns for symbols that appear in zero rows
//! - User status only takes space when present
//!
//! **Maintains alignment:**
//! - All symbols align vertically at their positions (vertical scannability)
//! - Grid adapts to minimize width based on active positions
//!
//! # Priority System Design
//!
//! ## Priority Scoring Model
//!
//! The allocation system uses a **priority scoring model**:
//! ```text
//! final_priority = base_priority + empty_penalty
//! ```
//!
//! **Base priorities** (1-11) are determined by **user need hierarchy** - what questions users need
//! answered when scanning worktrees:
//! - 1: Branch (identity - "what is this?")
//! - 2: Working diff (critical - "do I need to commit?")
//! - 3: Ahead/behind (critical - "am I out of sync?")
//! - 4-10: Context (work volume, states, path, time, CI, etc.)
//! - 11: Message (nice-to-have, space-hungry)
//!
//! **Empty penalty**: +10 if column has no data (only header)
//! - Empty working_diff: 2 + 10 = priority 12
//! - Empty ahead/behind: 3 + 10 = priority 13
//! - etc.
//!
//! This creates two effective priority tiers:
//! - **Tier 1 (priorities 1-11)**: Columns with actual data
//! - **Tier 2 (priorities 12-21)**: Empty columns (visual consistency)
//!
//! The empty penalty is large (+10) but not infinite, so empty columns maintain their relative
//! ordering (empty working_diff still ranks higher than empty ci_status) for visual consistency.
//!
//! ## Why This Design?
//!
//! **Problem**: Terminal width is limited. We must decide what to show.
//!
//! **Goals**:
//! 1. Show critical data (uncommitted changes, sync status) at any terminal width
//! 2. Show nice-to-have data (message, commit hash) when space allows
//! 3. Maintain visual consistency - empty columns in predictable positions at wide widths
//!
//! **Key decision**: Message sits at the boundary (priority 11). Empty columns (priority 12+)
//! rank below message, so:
//! - Narrow terminals: Data columns + message (hide empty columns)
//! - Wide terminals: Data columns + message + empty columns (visual consistency)
//!
//! ## Special Cases
//!
//! Three columns have non-standard behavior that extends beyond the basic two-tier model:
//!
//! 1. **BranchDiff** - Visibility gate (`show_full` flag)
//!    - Hidden by default as too noisy for typical usage
//!    - Only allocated when `show_full=true` (match guard skips if false)
//!
//! 2. **CiStatus** - Visibility gate (`fetch_ci` flag)
//!    - Only shown when `fetch_ci=true` (when CI data was requested)
//!    - Bypasses the tier system entirely when `fetch_ci=false`
//!    - Within the visibility gate, follows normal two-tier priority (priority 9 with data, 19 when empty)
//!
//! 3. **Message** - Flexible sizing with post-allocation expansion
//!    - Allocated at priority 11 with flexible width (min 20, preferred 50)
//!    - After all columns allocated (including empty ones), expands up to max 100 using leftover space
//!    - Two-step process ensures critical columns get space before message grows
//!
//! ## Implementation
//!
//! The code implements this using a centralized registry and priority-based allocation:
//!
//! ```rust
//! // Build candidates from centralized COLUMN_SPECS registry
//! let mut candidates: Vec<ColumnCandidate> = COLUMN_SPECS
//!     .iter()
//!     .filter(|spec| /* visibility gates: show_full, fetch_ci */)
//!     .map(|spec| ColumnCandidate {
//!         spec,
//!         priority: if spec.kind.has_data(&data_flags) {
//!             spec.base_priority
//!         } else {
//!             spec.base_priority + EMPTY_PENALTY
//!         }
//!     })
//!     .collect();
//!
//! // Sort by final priority
//! candidates.sort_by_key(|candidate| candidate.priority);
//!
//! // Allocate columns in priority order, building pending list
//! for candidate in candidates {
//!     if candidate.spec.kind == ColumnKind::Message {
//!         // Special handling: flexible width (min 20, preferred 50)
//!     } else if let Some(ideal) = ideal_for_column(candidate.spec, ...) {
//!         if let allocated = try_allocate(&mut remaining, ideal.width, ...) {
//!             pending.push(PendingColumn { spec: candidate.spec, width: allocated, format: ideal.format });
//!         }
//!     }
//! }
//!
//! // Message post-allocation expansion (uses truly leftover space)
//! if let Some(message_col) = pending.iter_mut().find(|col| col.spec.kind == ColumnKind::Message) {
//!     message_col.width += remaining.min(MAX_MESSAGE - message_col.width);
//! }
//! ```
//!
//! **Benefits**:
//! - Column metadata centralized in `COLUMN_SPECS` registry (single source of truth)
//! - Priority calculation explicit (base_priority + conditional EMPTY_PENALTY)
//! - Single unified allocation loop (no phase duplication)
//! - Easy to understand: build candidates → sort by priority → allocate → expand message
//! - Extensible: can add new modifiers (terminal width bonus, user config) without restructuring
//!
//! ## Helper Functions
//!
//! - `calculate_diff_width()`: Computes width for diff-style columns ("+added -deleted")
//! - `fit_header()`: Ensures column width ≥ header width to prevent overflow
//! - `try_allocate()`: Attempts to allocate space, returns 0 if insufficient

use crate::display::{find_common_prefix, get_terminal_width};
use anstyle::Style;
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;
use worktrunk::styling::{ADDITION, DELETION};

use super::{
    columns::{COLUMN_SPECS, ColumnKind, ColumnSpec, DiffVariant},
    model::ListItem,
};

/// Width of short commit hash display (first 8 hex characters)
const COMMIT_HASH_WIDTH: usize = 8;

/// Column header labels - single source of truth for all column headers.
/// Both layout calculations and rendering use these constants.
pub const HEADER_BRANCH: &str = "Branch";
pub const HEADER_STATUS: &str = "Status";
pub const HEADER_WORKING_DIFF: &str = "HEAD±";
pub const HEADER_AHEAD_BEHIND: &str = "main↕";
pub const HEADER_BRANCH_DIFF: &str = "main…±";
pub const HEADER_PATH: &str = "Path";
pub const HEADER_UPSTREAM: &str = "Remote⇅";
pub const HEADER_AGE: &str = "Age";
pub const HEADER_CI: &str = "CI";
pub const HEADER_COMMIT: &str = "Commit";
pub const HEADER_MESSAGE: &str = "Message";

/// Calculate maximum display width for a value using compact notation.
/// Returns the character width (including suffix) when the value is formatted.
///
/// Invariant: All values return either 1 or 2, ensuring consistent column width.
///
/// Examples: 0 -> 1, 5 -> 1, 42 -> 2, 100 -> 2 (displays as "1C"), 1000 -> 2 (displays as "1K")
fn max_display_width(value: usize) -> usize {
    if value < 10 {
        1 // Single digit (0-9)
    } else {
        2 // All values >= 10 display as 2 chars (either "42" or "4C" or "4K")
    }
}

/// Ensures a column width is at least as wide as its header.
///
/// This is the general solution for preventing header overflow: pass the header
/// string and the calculated data width, and this returns the larger of the two.
///
/// For empty columns (data_width = 0), returns header width. This allows empty
/// columns to be allocated at low priority (base_priority + EMPTY_PENALTY) for
/// visual consistency on wide terminals.
fn fit_header(header: &str, data_width: usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    data_width.max(header.width())
}

/// Calculates width for a diff-style column (format: "+added -deleted" or "↑ahead ↓behind").
///
/// Returns DiffWidths with:
/// - total: width including header minimum ("+{added} -{deleted}"), or just header width if no data
/// - added_digits/deleted_digits: number of digits for each part
///
/// Empty columns (both digits = 0) get header width and are allocated at low priority
/// (base_priority + EMPTY_PENALTY) for visual consistency on wide terminals.
fn calculate_diff_width(header: &str, added_digits: usize, deleted_digits: usize) -> DiffWidths {
    let has_data = added_digits > 0 || deleted_digits > 0;
    let data_width = if has_data {
        1 + added_digits + 1 + 1 + deleted_digits // "+added -deleted"
    } else {
        0 // fit_header will use header width for empty columns
    };
    let total = fit_header(header, data_width);

    DiffWidths {
        total,
        added_digits,
        deleted_digits,
    }
}

/// Helper: Try to allocate space for a column. Returns the allocated width if successful.
/// Updates `remaining` by subtracting the allocated width + spacing.
/// If is_first is true, doesn't require spacing before the column.
///
/// The spacing is consumed from the budget (subtracted from `remaining`) but not returned
/// as part of the column's width, since the spacing appears before the column content.
fn try_allocate(
    remaining: &mut usize,
    ideal_width: usize,
    spacing: usize,
    is_first: bool,
) -> usize {
    if ideal_width == 0 {
        return 0;
    }
    let required = if is_first {
        ideal_width
    } else {
        ideal_width + spacing // Gap before column + column content
    };
    if *remaining < required {
        return 0;
    }
    *remaining = remaining.saturating_sub(required);
    ideal_width // Return just the column width
}

/// Width information for two-part columns: diffs ("+128 -147") and arrows ("↑6 ↓1")
/// - For diff columns: added_digits/deleted_digits refer to line change counts
/// - For arrow columns: added_digits/deleted_digits refer to ahead/behind commit counts
#[derive(Clone, Copy, Debug)]
pub struct DiffWidths {
    pub total: usize,
    pub added_digits: usize,   // First part: + for diffs, ↑ for arrows
    pub deleted_digits: usize, // Second part: - for diffs, ↓ for arrows
}

#[derive(Clone, Debug)]
pub struct ColumnWidths {
    pub branch: usize,
    pub status: usize, // Includes both git status symbols and user-defined status
    pub time: usize,
    pub ci_status: usize,
    pub message: usize,
    pub ahead_behind: DiffWidths,
    pub working_diff: DiffWidths,
    pub branch_diff: DiffWidths,
    pub upstream: DiffWidths,
}

/// Tracks which columns have actual data (vs just headers)
#[derive(Clone, Copy, Debug)]
pub struct ColumnDataFlags {
    pub status: bool, // True if any item has git status symbols or user-defined status
    pub working_diff: bool,
    pub ahead_behind: bool,
    pub branch_diff: bool,
    pub upstream: bool,
    pub ci_status: bool,
}

/// Layout metadata including position mask for Status column
#[derive(Clone, Debug)]
pub struct LayoutMetadata {
    pub widths: ColumnWidths,
    pub data_flags: ColumnDataFlags,
    pub status_position_mask: super::model::PositionMask,
}

const EMPTY_PENALTY: u8 = 10;

#[derive(Clone, Copy, Debug)]
pub struct DiffDigits {
    pub added: usize,
    pub deleted: usize,
}

impl From<DiffWidths> for DiffDigits {
    fn from(widths: DiffWidths) -> Self {
        Self {
            added: widths.added_digits,
            deleted: widths.deleted_digits,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DiffDisplayConfig {
    pub variant: DiffVariant,
    pub positive_style: Style,
    pub negative_style: Style,
    pub always_show_zeros: bool,
}

impl ColumnKind {
    pub fn diff_display_config(self) -> Option<DiffDisplayConfig> {
        match self {
            ColumnKind::WorkingDiff | ColumnKind::BranchDiff => Some(DiffDisplayConfig {
                variant: DiffVariant::Signs,
                positive_style: ADDITION,
                negative_style: DELETION,
                always_show_zeros: false,
            }),
            ColumnKind::AheadBehind => Some(DiffDisplayConfig {
                variant: DiffVariant::Arrows,
                positive_style: ADDITION,
                negative_style: DELETION.dimmed(),
                always_show_zeros: false,
            }),
            ColumnKind::Upstream => Some(DiffDisplayConfig {
                variant: DiffVariant::Arrows,
                positive_style: ADDITION,
                negative_style: DELETION.dimmed(),
                always_show_zeros: true,
            }),
            _ => None,
        }
    }

    pub fn has_data(self, flags: &ColumnDataFlags) -> bool {
        match self {
            ColumnKind::Branch => true,
            ColumnKind::Status => flags.status,
            ColumnKind::WorkingDiff => flags.working_diff,
            ColumnKind::AheadBehind => flags.ahead_behind,
            ColumnKind::BranchDiff => flags.branch_diff,
            ColumnKind::Path => true,
            ColumnKind::Upstream => flags.upstream,
            ColumnKind::Time => true,
            ColumnKind::CiStatus => flags.ci_status,
            ColumnKind::Commit => true,
            ColumnKind::Message => true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ColumnFormat {
    Text,
    Diff(DiffColumnConfig),
}

#[derive(Clone, Copy, Debug)]
pub struct DiffColumnConfig {
    pub digits: DiffDigits,
    pub total_width: usize,
    pub display: DiffDisplayConfig,
}

#[derive(Clone, Debug)]
pub struct ColumnLayout {
    pub kind: ColumnKind,
    pub header: &'static str,
    pub start: usize,
    pub width: usize,
    pub format: ColumnFormat,
}

pub struct LayoutConfig {
    pub columns: Vec<ColumnLayout>,
    pub common_prefix: PathBuf,
    pub max_message_len: usize,
    pub hidden_nonempty_count: usize,
    pub status_position_mask: super::model::PositionMask,
}

#[derive(Clone, Copy, Debug)]
struct ColumnIdeal {
    width: usize,
    format: ColumnFormat,
}

impl ColumnIdeal {
    fn text(width: usize) -> Option<Self> {
        if width == 0 {
            None
        } else {
            Some(Self {
                width,
                format: ColumnFormat::Text,
            })
        }
    }

    fn diff(widths: DiffWidths, kind: ColumnKind) -> Option<Self> {
        if widths.total == 0 {
            return None;
        }

        let display = kind.diff_display_config()?;

        Some(Self {
            width: widths.total,
            format: ColumnFormat::Diff(DiffColumnConfig {
                digits: widths.into(),
                total_width: widths.total,
                display,
            }),
        })
    }
}

#[derive(Clone, Copy)]
struct ColumnCandidate<'a> {
    spec: &'a ColumnSpec,
    priority: u8,
}

#[derive(Clone, Copy)]
struct PendingColumn<'a> {
    spec: &'a ColumnSpec,
    width: usize,
    format: ColumnFormat,
}

fn ideal_for_column(
    spec: &ColumnSpec,
    widths: &ColumnWidths,
    max_path_width: usize,
    commit_width: usize,
) -> Option<ColumnIdeal> {
    match spec.kind {
        ColumnKind::Branch => ColumnIdeal::text(widths.branch),
        ColumnKind::Status => ColumnIdeal::text(widths.status),
        ColumnKind::Path => ColumnIdeal::text(max_path_width),
        ColumnKind::Time => ColumnIdeal::text(widths.time),
        ColumnKind::CiStatus => ColumnIdeal::text(widths.ci_status),
        ColumnKind::Commit => ColumnIdeal::text(commit_width),
        ColumnKind::Message => None,
        ColumnKind::WorkingDiff => ColumnIdeal::diff(widths.working_diff, ColumnKind::WorkingDiff),
        ColumnKind::AheadBehind => ColumnIdeal::diff(widths.ahead_behind, ColumnKind::AheadBehind),
        ColumnKind::BranchDiff => ColumnIdeal::diff(widths.branch_diff, ColumnKind::BranchDiff),
        ColumnKind::Upstream => ColumnIdeal::diff(widths.upstream, ColumnKind::Upstream),
    }
}

pub fn calculate_column_widths(items: &[ListItem], fetch_ci: bool) -> LayoutMetadata {
    // Track maximum data widths (headers are enforced via fit_header() later)
    let mut max_branch = 0;
    let mut max_time = 0;
    let mut max_message = 0;

    // Track diff component widths separately
    let mut max_wt_added_digits = 0;
    let mut max_wt_deleted_digits = 0;
    let mut max_br_added_digits = 0;
    let mut max_br_deleted_digits = 0;

    // Track ahead/behind digit widths separately for alignment
    let mut max_ahead_digits = 0;
    let mut max_behind_digits = 0;
    let mut max_upstream_ahead_digits = 0;
    let mut max_upstream_behind_digits = 0;

    // Track which status positions are used across all items
    // Always use PositionMask::FULL for consistent spacing with progressive mode
    // (progressive mode uses FULL because it doesn't have all data upfront)

    for item in items {
        let commit = item.commit_details();
        let counts = item.counts();
        let branch_diff = item.branch_diff().diff;
        let upstream = item.upstream();
        let worktree_info = item.worktree_data();

        // Branch name
        max_branch = max_branch.max(item.branch_name().width());

        // Status column: git status symbols (worktrees only)
        // Note: We always use PositionMask::FULL for rendering (see below),
        // so we don't need to collect position masks from items anymore.

        // Time
        let time_str = crate::display::format_relative_time(commit.timestamp);
        max_time = max_time.max(time_str.width());

        // Message (truncate to 50 chars max)
        let msg_len = commit.commit_message.chars().take(50).count();
        max_message = max_message.max(msg_len);

        // Ahead/behind (only for non-primary items) - track digits separately
        if !item.is_primary() && (counts.ahead > 0 || counts.behind > 0) {
            max_ahead_digits = max_ahead_digits.max(max_display_width(counts.ahead));
            max_behind_digits = max_behind_digits.max(max_display_width(counts.behind));
        }

        // Working tree diff (worktrees only) - track digits separately
        if let Some(info) = worktree_info
            && let Some(ref working_tree_diff) = info.working_tree_diff
            && !working_tree_diff.is_empty()
        {
            max_wt_added_digits =
                max_wt_added_digits.max(max_display_width(working_tree_diff.added));
            max_wt_deleted_digits =
                max_wt_deleted_digits.max(max_display_width(working_tree_diff.deleted));
        }

        // Branch diff (only for non-primary items) - track digits separately
        if !item.is_primary() && !branch_diff.is_empty() {
            max_br_added_digits = max_br_added_digits.max(max_display_width(branch_diff.added));
            max_br_deleted_digits =
                max_br_deleted_digits.max(max_display_width(branch_diff.deleted));
        }

        // Upstream tracking - track digits only (not remote name yet)
        if let Some((_remote_name, upstream_ahead, upstream_behind)) = upstream.active() {
            max_upstream_ahead_digits =
                max_upstream_ahead_digits.max(max_display_width(upstream_ahead));
            max_upstream_behind_digits =
                max_upstream_behind_digits.max(max_display_width(upstream_behind));
        }
    }

    // Calculate diff widths using helper (format: "+left -right")
    let working_diff = calculate_diff_width(
        HEADER_WORKING_DIFF,
        max_wt_added_digits,
        max_wt_deleted_digits,
    );
    let branch_diff = calculate_diff_width(
        HEADER_BRANCH_DIFF,
        max_br_added_digits,
        max_br_deleted_digits,
    );
    let ahead_behind =
        calculate_diff_width(HEADER_AHEAD_BEHIND, max_ahead_digits, max_behind_digits);

    // Upstream (format: "↑n ↓n", TODO: add remote name when show_remote_names is implemented)
    let upstream = calculate_diff_width(
        HEADER_UPSTREAM,
        max_upstream_ahead_digits,
        max_upstream_behind_digits,
    );

    // Status column: Must match PositionMask::FULL width for consistent alignment with progressive mode
    // PositionMask::FULL allocates: 5+1+1+1+1+1+2+2 = 14 chars
    // (working_tree=5, conflicts=1, git_op=1, main_div=1, upstream_div=1,
    //  branch_state=1, worktree_attrs=2, user_status=2)
    // This ensures buffered and progressive modes produce identical column layouts.
    //
    // User status is limited to single emoji or two characters (2 visual width allocation).
    let has_status_data = items.iter().any(|item| {
        item.status_symbols
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    });
    let final_status = fit_header(HEADER_STATUS, 14);

    // CI status column: Always 2 chars wide (single symbol "●")
    // Only show if we attempted to fetch CI data (regardless of whether any items have status)
    let has_ci_status = fetch_ci && items.iter().any(|item| item.pr_status().is_some());
    let ci_status_width = 2; // Fixed width

    let widths = ColumnWidths {
        branch: fit_header(HEADER_BRANCH, max_branch),
        status: final_status,
        time: fit_header(HEADER_AGE, max_time),
        ci_status: fit_header(HEADER_CI, ci_status_width),
        message: fit_header(HEADER_MESSAGE, max_message),
        ahead_behind,
        working_diff,
        branch_diff,
        upstream,
    };

    let data_flags = ColumnDataFlags {
        status: has_status_data,
        working_diff: working_diff.added_digits > 0 || working_diff.deleted_digits > 0,
        ahead_behind: ahead_behind.added_digits > 0 || ahead_behind.deleted_digits > 0,
        branch_diff: branch_diff.added_digits > 0 || branch_diff.deleted_digits > 0,
        upstream: upstream.added_digits > 0 || upstream.deleted_digits > 0,
        ci_status: has_ci_status,
    };

    LayoutMetadata {
        widths,
        data_flags,
        status_position_mask: super::model::PositionMask::FULL,
    }
}

/// Allocate columns using priority-based allocation logic.
///
/// This is the core allocation algorithm used by both `calculate_responsive_layout()`
/// (with actual data-based widths) and `calculate_layout_from_basics()` (with estimated widths).
fn allocate_columns_with_priority(
    metadata: &LayoutMetadata,
    show_full: bool,
    fetch_ci: bool,
    max_path_width: usize,
    commit_width: usize,
    terminal_width: usize,
    common_prefix: PathBuf,
) -> LayoutConfig {
    let spacing = 2;
    let mut remaining = terminal_width;

    // Build candidates with priorities
    let mut candidates: Vec<ColumnCandidate> = COLUMN_SPECS
        .iter()
        .filter(|spec| {
            (!spec.requires_show_full || show_full) && (!spec.requires_fetch_ci || fetch_ci)
        })
        .map(|spec| ColumnCandidate {
            spec,
            priority: if spec.kind.has_data(&metadata.data_flags) {
                spec.base_priority
            } else {
                spec.base_priority + EMPTY_PENALTY
            },
        })
        .collect();

    candidates.sort_by_key(|candidate| candidate.priority);

    // Store which candidates have data for later calculation of hidden columns
    let candidates_with_data: Vec<_> = candidates
        .iter()
        .map(|c| (c.spec.kind, c.spec.kind.has_data(&metadata.data_flags)))
        .collect();

    const MIN_MESSAGE: usize = 20;
    const PREFERRED_MESSAGE: usize = 50;
    const MAX_MESSAGE: usize = 100;

    let mut pending: Vec<PendingColumn> = Vec::new();

    // Allocate columns in priority order
    for candidate in candidates {
        let spec = candidate.spec;

        // Special handling for Message column
        if spec.kind == ColumnKind::Message {
            let is_first = pending.is_empty();
            let spacing_cost = if is_first { 0 } else { spacing };

            if remaining <= spacing_cost {
                continue;
            }

            let available = remaining - spacing_cost;
            let mut message_width = 0;

            if available >= PREFERRED_MESSAGE {
                message_width = PREFERRED_MESSAGE.min(metadata.widths.message);
            } else if available >= MIN_MESSAGE {
                message_width = available.min(metadata.widths.message);
            }

            if message_width > 0 {
                remaining = remaining.saturating_sub(message_width + spacing_cost);
                pending.push(PendingColumn {
                    spec,
                    width: message_width,
                    format: ColumnFormat::Text,
                });
            }

            continue;
        }

        // For non-message columns
        let Some(ideal) = ideal_for_column(spec, &metadata.widths, max_path_width, commit_width)
        else {
            continue;
        };

        let allocated = try_allocate(&mut remaining, ideal.width, spacing, pending.is_empty());
        if allocated > 0 {
            pending.push(PendingColumn {
                spec,
                width: allocated,
                format: ideal.format,
            });
        }
    }

    // Expand message column with leftover space
    let mut max_message_len = 0;
    if let Some(message_col) = pending
        .iter_mut()
        .find(|col| col.spec.kind == ColumnKind::Message)
    {
        if message_col.width < MAX_MESSAGE && remaining > 0 {
            let expansion = remaining.min(MAX_MESSAGE - message_col.width);
            message_col.width += expansion;
        }
        max_message_len = message_col.width;
    }

    // Sort by display index to maintain correct visual order
    pending.sort_by_key(|col| col.spec.display_index);

    // Build final column layouts with positions
    let gap = 2;
    let mut position = 0;
    let mut columns = Vec::new();

    for col in pending {
        let start = if columns.is_empty() {
            0
        } else {
            position + gap
        };
        position = start + col.width;

        columns.push(ColumnLayout {
            kind: col.spec.kind,
            header: col.spec.header,
            start,
            width: col.width,
            format: col.format,
        });
    }

    // Count how many non-empty columns were hidden (not allocated)
    let allocated_kinds: std::collections::HashSet<_> =
        columns.iter().map(|col| col.kind).collect();
    let hidden_nonempty_count = candidates_with_data
        .iter()
        .filter(|(kind, has_data)| !allocated_kinds.contains(kind) && *has_data)
        .count();

    LayoutConfig {
        columns,
        common_prefix,
        max_message_len,
        hidden_nonempty_count,
        status_position_mask: metadata.status_position_mask,
    }
}

/// Calculate responsive layout based on terminal width
pub fn calculate_responsive_layout(
    items: &[ListItem],
    show_full: bool,
    fetch_ci: bool,
) -> LayoutConfig {
    let terminal_width = get_terminal_width();
    let paths: Vec<&Path> = items
        .iter()
        .filter_map(|item| item.worktree_path().map(|path| path.as_path()))
        .collect();
    let common_prefix = find_common_prefix(&paths);

    // Calculate ideal column widths and track which columns have data
    let metadata = calculate_column_widths(items, fetch_ci);

    // Calculate actual maximum path width (after common prefix removal)
    let path_data_width = items
        .iter()
        .filter_map(|item| item.worktree_path())
        .map(|path| {
            use crate::display::shorten_path;
            use unicode_width::UnicodeWidthStr;
            shorten_path(path.as_path(), &common_prefix).width()
        })
        .max()
        .unwrap_or(0);
    let max_path_width = fit_header(HEADER_PATH, path_data_width);

    let commit_width = fit_header(HEADER_COMMIT, COMMIT_HASH_WIDTH);

    allocate_columns_with_priority(
        &metadata,
        show_full,
        fetch_ci,
        max_path_width,
        commit_width,
        terminal_width,
        common_prefix,
    )
}

/// Calculate responsive layout from basic worktree info with estimated column widths.
///
/// This is used for progressive rendering where we want to show the layout immediately
/// before collecting full data. We use:
/// - Actual branch names and paths from worktree list (fast operation)
/// - Branch names from branches without worktrees (when --branches is used)
/// - Estimated widths for columns that require expensive git operations
///
/// The estimates are generous to minimize truncation:
/// - Status: 5 chars (covers "≡ 🔧" and most common patterns)
/// - HEAD±: 6 chars (covers "+12 -5" and similar)
/// - main↕: 6 chars (covers "↑23 ↓5")
/// - Remote⇅: 7 chars (header width, covers most cases)
/// - Age: 15 chars (covers "3 months ago")
/// - Commit: 8 chars (fixed)
/// - Message: flexible (20-100 chars)
///
/// Note: This is work-in-progress for improved progressive rendering.
pub fn calculate_layout_from_basics(
    worktrees: &[worktrunk::git::Worktree],
    branch_names: &[String],
    show_full: bool,
    fetch_ci: bool,
) -> LayoutConfig {
    let terminal_width = get_terminal_width();

    // Calculate common prefix from paths
    let paths: Vec<&Path> = worktrees.iter().map(|wt| wt.path.as_path()).collect();
    let common_prefix = find_common_prefix(&paths);

    // Calculate actual widths for things we know
    // Include both worktree branch names AND branch names without worktrees
    let max_worktree_branch = worktrees
        .iter()
        .filter_map(|wt| wt.branch.as_deref())
        .map(|b| b.width())
        .max()
        .unwrap_or(0);

    let max_standalone_branch = branch_names.iter().map(|b| b.width()).max().unwrap_or(0);

    let max_branch = fit_header(
        HEADER_BRANCH,
        max_worktree_branch.max(max_standalone_branch),
    );

    let path_data_width = worktrees
        .iter()
        .map(|wt| {
            use crate::display::shorten_path;
            shorten_path(wt.path.as_path(), &common_prefix).width()
        })
        .max()
        .unwrap_or(0);
    let max_path_width = fit_header(HEADER_PATH, path_data_width);

    // Fixed widths for slow columns (require expensive git operations)
    // These are predetermined widths, not based on actual data
    // Values exceeding these widths will be shown with K/M suffixes
    //
    // Status column: Must match PositionMask::FULL width for consistent alignment
    // PositionMask::FULL allocates: 5+1+1+1+1+1+2+2 = 14 chars
    // (working_tree=5, conflicts=1, git_op=1, main_div=1, upstream_div=1,
    //  branch_state=1, worktree_attrs=2, user_status=2)
    let status_fixed = fit_header(HEADER_STATUS, 14);
    let working_diff_fixed = fit_header(HEADER_WORKING_DIFF, 9); // "+999 -999" (9 chars)
    let ahead_behind_fixed = fit_header(HEADER_AHEAD_BEHIND, 7); // "↑99 ↓99" (7 chars)
    let branch_diff_fixed = fit_header(HEADER_BRANCH_DIFF, 9); // "+999 -999" (9 chars)
    let upstream_fixed = fit_header(HEADER_UPSTREAM, 7); // "↑99 ↓99" (7 chars)
    let age_estimate = 15; // "3 months ago" (fast to compute)
    let commit_width = fit_header(HEADER_COMMIT, COMMIT_HASH_WIDTH); // Fixed 8 chars
    // CI column shows only indicator symbol (●/○/◐)
    let ci_estimate = fit_header(HEADER_CI, 1);

    // For progressive rendering, assume columns will have data
    // (better to show and hide than to not show)
    let data_flags = ColumnDataFlags {
        status: true,
        working_diff: true,
        ahead_behind: true,
        branch_diff: show_full,
        upstream: true,
        ci_status: fetch_ci,
    };

    // Build ColumnWidths with fixed widths for slow columns
    let estimated_widths = ColumnWidths {
        branch: max_branch,
        status: status_fixed,
        time: age_estimate,
        ci_status: ci_estimate,
        message: 50, // Will be flexible later
        ahead_behind: DiffWidths {
            total: ahead_behind_fixed,
            added_digits: 2, // Compact notation at 100+
            deleted_digits: 2,
        },
        working_diff: DiffWidths {
            total: working_diff_fixed,
            added_digits: 2, // Compact notation at 100+
            deleted_digits: 2,
        },
        branch_diff: DiffWidths {
            total: branch_diff_fixed,
            added_digits: 2, // Compact notation at 100+
            deleted_digits: 2,
        },
        upstream: DiffWidths {
            total: upstream_fixed,
            added_digits: 2, // Compact notation at 100+
            deleted_digits: 2,
        },
    };

    // Use full position mask for progressive rendering
    // (we don't know which positions will be needed until data is collected)
    let status_position_mask = super::model::PositionMask::FULL;

    // Build metadata for allocation
    let metadata = LayoutMetadata {
        widths: estimated_widths,
        data_flags,
        status_position_mask,
    };

    allocate_columns_with_priority(
        &metadata,
        show_full,
        fetch_ci,
        max_path_width,
        commit_width,
        terminal_width,
        common_prefix,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::list::columns::ColumnKind;
    use std::path::PathBuf;
    use worktrunk::git::LineDiff;

    #[test]
    fn test_column_width_calculation_with_unicode() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, ItemKind, StatusSymbols,
            UpstreamStatus, WorktreeData,
        };

        let item1 = super::ListItem {
            head: "abc123".to_string(),
            branch: Some("main".to_string()),
            commit: Some(CommitDetails {
                timestamp: 0,
                commit_message: "Test".to_string(),
            }),
            counts: Some(AheadBehind {
                ahead: 3,
                behind: 2,
            }),
            branch_diff: Some(BranchDiffTotals {
                diff: LineDiff::from((200, 30)),
            }),
            upstream: Some(UpstreamStatus::from_parts(Some("origin".to_string()), 4, 0)),
            pr_status: None,
            has_conflicts: Some(false),
            user_status: None,
            status_symbols: Some(StatusSymbols::default()),
            display: DisplayFields::default(),
            kind: ItemKind::Worktree(Box::new(WorktreeData {
                path: PathBuf::from("/test"),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
                working_tree_diff: Some(LineDiff::from((100, 50))),
                working_tree_diff_with_main: Some(Some(LineDiff::default())),
                worktree_state: None,
                is_primary: false,
                working_tree_symbols: Some(String::new()),
                is_dirty: Some(false),
                working_diff_display: None,
            })),
        };

        let metadata = calculate_column_widths(&[item1], false);
        let widths = metadata.widths;

        // "↑3 ↓2" has format "↑3 ↓2" = 1+1+1+1+1 = 5, header "main↕" is also 5
        assert_eq!(
            widths.ahead_behind.total, 5,
            "Ahead/behind column should fit header 'main↕' (width 5)"
        );
        assert_eq!(widths.ahead_behind.added_digits, 1, "3 displays as 1 digit");
        assert_eq!(
            widths.ahead_behind.deleted_digits, 1,
            "2 displays as 1 digit"
        );

        // "+1C -50" has width 7 (compact notation), header "HEAD±" is 5, so column width is 7
        assert_eq!(
            widths.working_diff.total, 7,
            "Working diff column should fit data '+1C -50' (width 7)"
        );
        assert_eq!(
            widths.working_diff.added_digits, 2,
            "100 displays as 2 digits (1C)"
        );
        assert_eq!(
            widths.working_diff.deleted_digits, 2,
            "50 displays as 2 digits"
        );

        // "+2C -30" has width 7 (compact notation), header "main…±" is 6, so column width is 7
        assert_eq!(
            widths.branch_diff.total, 7,
            "Branch diff column should fit data '+2C -30' (width 7)"
        );
        assert_eq!(
            widths.branch_diff.added_digits, 2,
            "200 displays as 2 digits (2C)"
        );
        assert_eq!(
            widths.branch_diff.deleted_digits, 2,
            "30 displays as 2 digits"
        );

        // Upstream: "↑4 ↓0" = "↑" (1) + "4" (1) + " " (1) + "↓" (1) + "0" (1) = 5, but header "Remote⇅" = 7
        assert_eq!(
            widths.upstream.total, 7,
            "Upstream column should fit header 'Remote⇅' (width 7)"
        );
        assert_eq!(widths.upstream.added_digits, 1, "4 has 1 digit");
        assert_eq!(widths.upstream.deleted_digits, 1, "0 has 1 digit");
    }

    #[test]
    fn test_max_display_width_edge_cases() {
        // Single digits (0-9) display as 1 character
        assert_eq!(max_display_width(0), 1, "0 displays as '0' (1 char)");
        assert_eq!(max_display_width(5), 1, "5 displays as '5' (1 char)");
        assert_eq!(max_display_width(9), 1, "9 displays as '9' (1 char)");

        // Two digits (10-99) display as 2 characters
        assert_eq!(max_display_width(10), 2, "10 displays as '10' (2 chars)");
        assert_eq!(max_display_width(42), 2, "42 displays as '42' (2 chars)");
        assert_eq!(max_display_width(99), 2, "99 displays as '99' (2 chars)");

        // Hundreds (100-999) display as 2 characters with C suffix
        assert_eq!(max_display_width(100), 2, "100 displays as '1C' (2 chars)");
        assert_eq!(max_display_width(648), 2, "648 displays as '6C' (2 chars)");
        assert_eq!(max_display_width(999), 2, "999 displays as '9C' (2 chars)");

        // Thousands (1000-9999) display as 2 characters with K suffix
        assert_eq!(
            max_display_width(1000),
            2,
            "1000 displays as '1K' (2 chars)"
        );
        assert_eq!(
            max_display_width(1500),
            2,
            "1500 displays as '1K' (2 chars)"
        );
        assert_eq!(
            max_display_width(9999),
            2,
            "9999 displays as '9K' (2 chars)"
        );

        // Large values (10000+) cap at 2 characters
        assert_eq!(
            max_display_width(10000),
            2,
            "10000 displays as '9K' (capped at 2 chars)"
        );
        assert_eq!(
            max_display_width(100000),
            2,
            "100000 displays as '9K' (capped at 2 chars)"
        );
    }

    #[test]
    fn test_visible_columns_follow_gap_rule() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, ItemKind, StatusSymbols,
            UpstreamStatus, WorktreeData,
        };

        // Create test data with specific widths to verify position calculation
        let item = super::ListItem {
            head: "abc12345".to_string(),
            branch: Some("feature".to_string()),
            commit: Some(CommitDetails {
                timestamp: 1234567890,
                commit_message: "Test commit message".to_string(),
            }),
            counts: Some(AheadBehind {
                ahead: 5,
                behind: 10,
            }),
            branch_diff: Some(BranchDiffTotals {
                diff: LineDiff::from((200, 30)),
            }),
            upstream: Some(UpstreamStatus::from_parts(Some("origin".to_string()), 4, 2)),
            pr_status: None,
            has_conflicts: Some(false),
            user_status: None,
            status_symbols: Some(StatusSymbols::default()),
            display: DisplayFields::default(),
            kind: ItemKind::Worktree(Box::new(WorktreeData {
                path: PathBuf::from("/test/path"),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
                working_tree_diff: Some(LineDiff::from((100, 50))),
                working_tree_diff_with_main: Some(Some(LineDiff::default())),
                worktree_state: None,
                is_primary: false,
                working_tree_symbols: Some(String::new()),
                is_dirty: Some(false),
                working_diff_display: None,
            })),
        };

        let items = vec![item];
        let layout = calculate_responsive_layout(&items, false, false);

        assert!(
            !layout.columns.is_empty(),
            "At least one column should be visible"
        );

        let mut columns_iter = layout.columns.iter();
        let first = columns_iter.next().expect("branch column should exist");
        assert_eq!(
            first.kind,
            ColumnKind::Branch,
            "Branch column should be first"
        );
        assert_eq!(first.start, 0, "Branch should begin at position 0");

        let mut previous_end = first.start + first.width;
        for column in columns_iter {
            assert_eq!(
                column.start,
                previous_end + 2,
                "Columns should be separated by a 2-space gap"
            );
            previous_end = column.start + column.width;
        }

        let path_column = layout
            .columns
            .iter()
            .find(|col| col.kind == ColumnKind::Path)
            .expect("Path column must be present");
        assert!(path_column.width > 0, "Path column must have width > 0");
    }

    #[test]
    fn test_column_positions_with_empty_columns() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, ItemKind, StatusSymbols,
            UpstreamStatus, WorktreeData,
        };

        // Create minimal data - most columns will be empty
        let item = super::ListItem {
            head: "abc12345".to_string(),
            branch: Some("main".to_string()),
            commit: Some(CommitDetails {
                timestamp: 1234567890,
                commit_message: "Test".to_string(),
            }),
            counts: Some(AheadBehind {
                ahead: 0,
                behind: 0,
            }),
            branch_diff: Some(BranchDiffTotals {
                diff: LineDiff::default(),
            }),
            upstream: Some(UpstreamStatus::default()),
            pr_status: None,
            has_conflicts: Some(false),
            user_status: None,
            status_symbols: Some(StatusSymbols::default()),
            display: DisplayFields::default(),
            kind: ItemKind::Worktree(Box::new(WorktreeData {
                path: PathBuf::from("/test"),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
                working_tree_diff: Some(LineDiff::default()),
                working_tree_diff_with_main: Some(Some(LineDiff::default())),
                worktree_state: None,
                is_primary: true, // Primary worktree: no ahead/behind shown
                working_tree_symbols: Some(String::new()),
                is_dirty: Some(false),
                working_diff_display: None,
            })),
        };

        let items = vec![item];
        let layout = calculate_responsive_layout(&items, false, false);

        assert!(
            layout
                .columns
                .first()
                .map(|col| col.kind == ColumnKind::Branch && col.start == 0)
                .unwrap_or(false),
            "Branch column should start at position 0"
        );

        // Columns with data should always be visible (Branch, Path, Time, Commit, Message)
        let path_visible = layout
            .columns
            .iter()
            .any(|col| col.kind == ColumnKind::Path);
        assert!(path_visible, "Path should always be visible (has data)");

        // Empty columns may or may not be visible depending on terminal width
        // They have low priority (base_priority + EMPTY_PENALTY) so they're allocated
        // only if space remains after higher-priority columns
    }

    #[test]
    fn test_consecutive_empty_columns_have_low_priority() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, ItemKind, StatusSymbols,
            UpstreamStatus, WorktreeData,
        };

        // Create data where multiple consecutive columns are empty:
        // visible(branch) → empty(working_diff) → empty(ahead_behind) → empty(branch_diff)
        // → empty(states) → visible(path)
        let item = super::ListItem {
            head: "abc12345".to_string(),
            branch: Some("feature-x".to_string()),
            commit: Some(CommitDetails {
                timestamp: 1234567890,
                commit_message: "Test commit".to_string(),
            }),
            counts: Some(AheadBehind {
                ahead: 0,
                behind: 0,
            }),
            branch_diff: Some(BranchDiffTotals {
                diff: LineDiff::default(),
            }), // Empty: no diff
            upstream: Some(UpstreamStatus::default()), // Empty: no upstream
            pr_status: None,
            has_conflicts: Some(false),
            user_status: None,
            status_symbols: Some(StatusSymbols::default()),
            display: DisplayFields::default(),
            kind: ItemKind::Worktree(Box::new(WorktreeData {
                path: PathBuf::from("/test/worktree"),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
                working_tree_diff: Some(LineDiff::default()), // Empty: no dirty changes
                working_tree_diff_with_main: Some(Some(LineDiff::default())),
                worktree_state: None, // Empty: no state
                is_primary: true,     // Empty: no ahead/behind for primary
                working_tree_symbols: Some(String::new()),
                is_dirty: Some(false),
                working_diff_display: None,
            })),
        };

        let items = vec![item];
        let layout = calculate_responsive_layout(&items, false, false);

        let path_visible = layout
            .columns
            .iter()
            .any(|col| col.kind == ColumnKind::Path);
        assert!(
            path_visible,
            "Path should be visible (has data, priority 7)"
        );

        let message_visible = layout
            .columns
            .iter()
            .any(|col| col.kind == ColumnKind::Message);
        assert!(
            message_visible,
            "Message should be allocated before empty columns (priority 12 < empty columns)"
        );

        // Empty columns (priority 12+) may or may not be visible depending on terminal width.
        // They rank lower than message (priority 12), so message allocates first.
    }
}

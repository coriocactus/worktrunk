# Improving `wt select`: Interactive Worktree Selector Interface

## Executive Summary

We have implemented a basic interactive worktree selector (`wt select`) using the skim fuzzy finder library. The command works functionally - users can fuzzy search through worktrees and switch to them - but we want to explore how to make this interface significantly more useful and powerful. This report documents the current implementation, available data, and open questions about how to design the best possible interface.

## Project Context

**Worktrunk** is a git worktree management tool written in Rust. Git worktrees allow multiple working directories for a single repository, enabling developers to work on multiple branches simultaneously without stashing changes or maintaining multiple clones.

The tool already has a `wt list` command that displays rich information about worktrees in a table format. The new `wt select` command provides an interactive fuzzy-finder interface for switching between worktrees.

## Goals

1. **Primary Goal**: Create an interactive interface that makes it faster and easier to navigate between worktrees compared to `wt list` + `wt switch <name>`
2. **Design Goal**: Surface the most relevant information to help users make switching decisions quickly
3. **UX Goal**: Support power users who may have dozens of worktrees and need efficient filtering/sorting
4. **Extensibility Goal**: Design an interface that could later support additional actions beyond just switching (e.g., remove, merge, create)

## Current Implementation

### Technology Stack

- **skim** (v0.20): Rust-based fuzzy finder library, similar to fzf
- **Bundled**: skim is compiled into the binary (~500KB increase), no external dependencies
- **Features Used**:
  - Basic fuzzy matching on display text
  - Preview pane (right 50% of screen)
  - Custom header with keyboard shortcuts
  - Single selection mode

### Current Interface Design

**Main Display (Left Pane)**:
Each line shows:
```
<branch_name>  <status_symbols>  <commit_message>
```

Example output:
```
main                           Initial commit
skim      !+                   Add skim-based selector
refactor  !                    Refactor list rendering
switch-   ?!                   WIP: switch improvements
demo-fix  ↓                    Fix demo crash
```

**Status Symbols** (8 characters wide, centered):
- `·` - Branch without worktree
- `=` - Merge conflicts in working tree
- `↑` - Ahead of main branch
- `↓` - Behind main branch
- `⇡` - Ahead of remote tracking branch
- `⇣` - Behind remote tracking branch
- `?` - Untracked files present
- `!` - Modified files (unstaged changes)
- `+` - Staged files
- `»` - Renamed files
- `✘` - Deleted files

**Preview Pane (Right 50%)**:
Shell script that shows:
1. Branch name header
2. Working tree status (`git status --short`)
3. Ahead/behind counts vs main
4. Recent 5 commits (`git log --oneline`)
5. Diff stats vs main (`git diff --stat`)
6. Diff preview (first 30 lines of `git diff`)

**Keyboard Shortcuts** (shown in header):
- `Enter` - Switch to selected worktree
- `Ctrl-/` - Toggle preview pane
- `Esc` - Cancel

### Code Structure

The implementation is in `src/commands/select.rs` (208 lines):

```rust
pub fn handle_select() -> Result<(), GitError> {
    let repo = Repository::current();

    // Gather data using existing list infrastructure
    let Some(list_data) = gather_list_data(&repo, false, false, false)? else {
        return Ok(());
    };

    // Calculate max branch name length for column alignment
    let max_branch_len = list_data.items.iter()
        .map(|item| item.branch_name().len())
        .max()
        .unwrap_or(20);

    // Convert to skim items with formatted display text
    let items: Vec<Arc<dyn SkimItem>> = list_data.items
        .into_iter()
        .map(|item| {
            let branch_name = item.branch_name().to_string();
            let commit_msg = item.commit_details()
                .commit_message
                .lines()
                .next()
                .unwrap_or("");

            // Aligned columns: branch + status + commit message
            let mut display_text = format!("{:<width$}", branch_name, width = max_branch_len);
            let status = if let Some(wt_info) = item.worktree_info() {
                format!("{:^8}", wt_info.status_symbols)
            } else {
                "        ".to_string()
            };
            display_text.push_str(&status);
            display_text.push_str("  ");
            display_text.push_str(commit_msg);

            Arc::new(WorktreeSkimItem { display_text, branch_name }) as Arc<dyn SkimItem>
        })
        .collect();

    // Configure skim with preview script
    let preview_cmd = r#"sh -c '
        BRANCH="$1"
        echo "━━━ Branch: $BRANCH ━━━"
        echo ""
        echo "Working tree:"
        git -c color.status=always status --short 2>/dev/null || echo "  Clean"
        # ... more preview commands
    ' -- {}"#.to_string();

    let options = SkimOptionsBuilder::default()
        .height("50%".to_string())
        .multi(false)
        .preview(Some(preview_cmd))
        .preview_window("right:50%".to_string())
        .header(Some("Enter: switch | Ctrl-/: toggle preview | Esc: cancel".to_string()))
        .build()?;

    // Run skim and handle selection
    let output = Skim::run_with(&options, Some(rx));
    if let Some(out) = output && !out.is_abort {
        if let Some(selected) = out.selected_items.first() {
            let identifier = selected.output().to_string();
            let config = WorktrunkConfig::load()?;
            let (result, resolved_branch) = handle_switch(&identifier, false, None, false, false, &config)?;
            handle_switch_output(&result, &resolved_branch, false)?;
        }
    }
    Ok(())
}
```

The `WorktreeSkimItem` implements skim's `SkimItem` trait:
- `text()` returns the formatted display string (what users see)
- `output()` returns the branch name (what gets selected)

### Available Data

The `gather_list_data()` function provides rich metadata for each worktree through the `ListItem` enum. Here's what's available:

```rust
pub enum ListItem {
    Worktree(WorktreeInfo),
    Branch(BranchInfo),  // Only when --branches flag is used
}

pub struct WorktreeInfo {
    // Core info
    pub worktree: worktrunk::git::Worktree,  // path, head, branch, locked, prunable
    pub is_primary: bool,

    // Commit info
    pub commit: CommitDetails {
        timestamp: i64,
        commit_message: String,
    },

    // Counts vs main branch
    pub counts: AheadBehind {
        ahead: usize,    // Commits ahead of main
        behind: usize,   // Commits behind main
    },

    // Working tree changes
    pub working_tree_diff: (usize, usize),  // (lines_added, lines_deleted) vs HEAD
    pub working_tree_diff_with_main: Option<(usize, usize)>,  // vs main branch

    // Branch diff stats
    pub branch_diff: BranchDiffTotals {
        diff: (usize, usize),  // (lines_added, lines_deleted) in commits ahead of main
    },

    // Remote tracking
    pub upstream: UpstreamStatus {
        remote: Option<String>,     // e.g. "origin"
        ahead: usize,                // Commits ahead of remote
        behind: usize,               // Commits behind remote
    },

    // State info
    pub worktree_state: Option<String>,  // e.g. "MERGING", "REBASING", "(matches main)"
    pub status_symbols: String,          // e.g. "!+↑⇡"
    pub has_conflicts: bool,

    // CI/PR info (requires --full flag)
    pub pr_status: Option<PrStatus> {
        state: String,        // "passed", "running", "failed", "conflicts", "no-ci"
        is_stale: bool,       // PR head differs from local
        pr_number: Option<i32>,
        pr_url: Option<String>,
    },

    // Display fields (for JSON output)
    pub display: DisplayFields,
    pub working_diff_display: Option<String>,
}
```

**Important Note**: Currently `gather_list_data(&repo, false, false, false)` is called with all flags set to `false`:
- First `false`: Don't include branches without worktrees
- Second `false`: Don't fetch CI status (expensive network requests)
- Third `false`: Don't check for merge conflicts (expensive git operations)

This means `pr_status` and `has_conflicts` are always `None`/`false` in the current implementation.

## What We Have Tried

### Iteration 1: Basic Implementation
- Used skim's default display
- No preview pane
- Result: Too minimal, users couldn't see enough context

### Iteration 2: Added Status Symbols
- Added the status symbols column (8 chars wide)
- Result: Better, but symbols are cryptic without reference

### Iteration 3: Added Commit Message
- Show first line of commit message after status symbols
- Result: Much better context for identifying branches

### Iteration 4: Added Preview Pane
- Implemented shell-based preview with git commands
- Shows working tree status, commits, diffs
- Result: Very helpful but preview is slow (runs git commands for each cursor movement)

### Iteration 5: Column Alignment
- Calculate max branch name length
- Use format strings for aligned columns
- Result: Professional appearance, easier to scan

### What Has Worked Well
1. **Fuzzy matching** - Users can type partial branch names and find worktrees quickly
2. **Status symbols** - Compact visual indicators (once users learn them)
3. **Commit message** - Essential for identifying what each branch is about
4. **Preview pane** - Very useful but has performance issues

### What Hasn't Worked Well
1. **Preview performance** - Preview runs git commands for every cursor movement, causing lag with many worktrees
2. **Limited information density** - Only showing branch name, status symbols, and commit message
3. **No sorting/filtering** - All worktrees shown in timestamp order (newest commit first)
4. **Static display** - Can't customize what columns are shown
5. **No color coding** - Main display is monochrome (preview uses git's colors)

## Current User Experience

**Positive Aspects**:
- Fast to launch
- Fuzzy search works well
- Preview provides detailed context
- Keyboard-driven workflow

**Pain Points**:
- Preview lag when rapidly moving through items
- Can't quickly identify which worktrees are "important" (e.g., have uncommitted work, are behind main)
- No visual distinction between clean branches and dirty branches
- Can't filter to "only show worktrees with changes"
- Branch names can be long, pushing commit messages off screen

## Open Questions & Research Opportunities

### 1. Display Design & Information Architecture

**Question**: What information is most valuable for making switching decisions?

**Current approach**: Branch name + status symbols + commit message

**Alternatives to research**:
- Should we show ahead/behind counts as numbers? (e.g., `↑3 ↓1` instead of just `↑↓`)
- Should we show working tree changes as numbers? (e.g., `+52 -12`)
- Should we use colors in the main display? (e.g., red for branches behind main, yellow for uncommitted changes)
- Should we show the worktree path? (currently only in preview)
- Should we show commit age? (e.g., "2d ago")
- Should we show PR status? (requires network calls)

**Research needed**:
- How do other fuzzy finders (fzf, telescope.nvim, etc.) handle information density?
- What do GitHub CLI (`gh`), GitLab CLI (`glab`) show in their interactive pickers?
- Are there UX studies on optimal information density in terminal interfaces?
- How do users prioritize information when choosing which branch to work on?

### 2. Preview Pane Performance

**Question**: How can we make the preview pane faster without sacrificing information?

**Current approach**: Shell script that runs 5-6 git commands per preview

**Problems**:
- Runs every time cursor moves to a new item
- Each git command has overhead (process spawn, repo traversal)
- With 20+ worktrees, moving through the list feels sluggish

**Alternatives to research**:
- **Pre-compute previews**: Generate all previews upfront, cache them
  - Trade-off: Slower startup, faster navigation
  - Question: How to keep cache fresh if user has multiple `wt` sessions?
- **Debounce previews**: Only update preview after cursor stops moving for N milliseconds
  - Trade-off: Faster navigation, delayed preview
  - Question: What's the optimal debounce delay?
- **Async previews**: Start preview computation in background, show "Loading..." placeholder
  - Trade-off: Complex implementation, better perceived performance
  - Question: Does skim support async preview updates?
- **Simplified previews**: Show less information (e.g., skip diff, just show status)
  - Trade-off: Faster but less useful
  - Question: What's the minimum viable preview?
- **Rust-based previews**: Instead of shell script, implement preview in Rust
  - Trade-off: More complex, potentially faster (no shell overhead)
  - Question: Can skim previews call back into Rust code?

**Research needed**:
- How does fzf handle preview performance with expensive commands?
- Do other fuzzy finders (telescope.nvim, peco, etc.) have solutions for this?
- What's the performance bottleneck: process spawning or git operations?
- Can we use libgit2 (via git2-rs) instead of shelling out to git?
- Does skim have built-in caching or preview optimization features?

### 3. Sorting & Filtering

**Question**: How should worktrees be sorted by default? Should users be able to filter?

**Current approach**: Sorted by commit timestamp (newest first)

**Sorting options to consider**:
- **By commit timestamp** (current) - Most recently updated branches at top
- **By creation time** - Order branches by when worktree was created
- **By name** - Alphabetical
- **By "importance"** - Custom heuristic (uncommitted changes > ahead of main > recently updated)
- **By ahead/behind counts** - Branches furthest from main first
- **By working tree diff** - Branches with most uncommitted changes first
- **User-configurable** - Let users choose in config file

**Filtering options to consider**:
- **Show only dirty** - Worktrees with uncommitted changes
- **Show only ahead** - Worktrees with commits not in main
- **Show only behind** - Worktrees behind main (need to sync)
- **Show only with conflicts** - Worktrees that would conflict with main
- **Smart filter** - Exclude "boring" worktrees (clean, no commits, matches main)
- **Keyboard toggle** - Press a key to toggle filter on/off

**Research needed**:
- How do other git tools (magit, lazygit, GitKraken) prioritize branches?
- What do users typically look for when choosing a branch to work on?
- Should filtering be done via skim's query syntax or dedicated keyboard shortcuts?
- Can we implement fuzzy filtering on metadata (e.g., `:dirty` to filter dirty worktrees)?

### 4. Color & Visual Hierarchy

**Question**: Should the main display use colors? How should visual hierarchy work?

**Current approach**: Monochrome display (no colors in main list)

**Alternatives**:
- **Color by state**: Red for conflicts, yellow for uncommitted, green for clean, blue for ahead
- **Color by age**: Dim old branches, bright for recent
- **Color by priority**: Custom heuristic (uncommitted > ahead > behind > clean)
- **Minimal color**: Only color status symbols, keep text monochrome
- **Full color**: Color branch names, symbols, and commit messages

**Visual hierarchy questions**:
- Should current worktree be highlighted? (bold, different color, marker character)
- Should primary worktree be visually distinct?
- Should "boring" branches be dimmed? (Like `wt list` does)
- Should we use icons/emoji beyond status symbols? (e.g., 🏠 for primary, ⭐ for current)

**Research needed**:
- What color schemes do other terminal UIs use? (lazygit, tig, delta)
- How do fuzzy finders handle colors? (fzf --ansi, telescope themes)
- What's the accessibility impact? (color blindness, low contrast terminals)
- Does skim support ANSI color codes in display text?

### 5. Multi-Action Support

**Question**: Should `wt select` support actions beyond switching?

**Current approach**: Single action (switch on Enter)

**Potential actions**:
- **Switch** (Enter) - Navigate to worktree
- **Remove** (Ctrl-X?) - Delete worktree and branch
- **Merge** (Ctrl-M?) - Merge worktree into main
- **Create** (Ctrl-C?) - Create new worktree from selected branch
- **Execute** (Ctrl-E?) - Run command in worktree
- **Preview toggle** (Ctrl-/) - Already implemented
- **Multi-select** (Tab?) - Select multiple worktrees for batch operations
- **Quick actions** - Show action menu for selected item

**Design questions**:
- Should actions be shown in the header?
- Should there be a "command palette" mode (press `:` to show actions)?
- Should multi-select be enabled? What batch operations make sense?
- Should we show different previews for different actions? (e.g., preview merge conflicts when Merge is selected)

**Research needed**:
- How do other fuzzy finders handle multi-action interfaces? (telescope.nvim actions, fzf --bind)
- What's the mental model for keyboard shortcuts? (vim-style, emacs-style, custom?)
- How to avoid overwhelming users with too many shortcuts?
- Should actions be discoverable (shown in UI) or documented (in help text)?

### 6. Skim-Specific Capabilities

**Question**: What skim features are we not using that could improve the interface?

**Skim features we're NOT using**:
- **Multi-select mode** - Allow selecting multiple worktrees
- **Custom scoring** - Override fuzzy match scoring
- **Preview position** - top/bottom/left/right
- **Preview window size** - Currently hardcoded to 50%
- **Colors** - Skim supports ANSI colors
- **Custom bindings** - Map keys to custom actions
- **Query history** - Remember previous searches
- **Default query** - Pre-populate search
- **Match highlighting** - Show which parts matched query

**Research needed**:
- What's the full skim API? (Check skim documentation/examples)
- What have fzf users found useful that we could port to skim?
- Are there skim-specific features that fzf doesn't have?
- How do skim's colors work with ANSI codes?
- Can we customize skim's UI beyond what we're using?

### 7. Performance & Scalability

**Question**: How does the interface scale to hundreds of worktrees?

**Current state**: Untested with large numbers of worktrees

**Concerns**:
- Preview performance degrades linearly with item count
- Display may become unwieldy with 50+ worktrees
- Fuzzy search performance with large item counts
- Memory usage with pre-computed data

**Research needed**:
- What's the performance of skim with 100+ items?
- Do we need pagination or lazy loading?
- Should we limit the number of worktrees shown by default?
- Can we profile where time is spent (data gathering vs display vs preview)?
- What's a realistic maximum number of worktrees users might have?

### 8. Integration with `wt list`

**Question**: Should `wt select` and `wt list` share more functionality?

**Current state**: `wt select` uses `gather_list_data()` but doesn't expose the same flags

**Possibilities**:
- Add `--full` flag to `wt select` for CI status / conflict detection
- Add `--branches` flag to include branches without worktrees
- Add `--format` flag for different display styles
- Share sorting/filtering logic between commands
- Make `wt list` interactive (arrow keys to select, press Enter to switch)

**Design questions**:
- Should `wt select` be a distinct tool or an interactive mode of `wt list`?
- What's the mental model: "list then select" or "select from list"?
- Should flags be consistent across commands or specialized per command?

**Research needed**:
- How do other CLIs handle the list vs interactive-select pattern?
- What's the user expectation when switching from `wt list` to `wt select`?
- Should there be a `wt list --interactive` flag instead of separate command?

## Assumptions We're Making

### Unproven Assumptions (High Risk)

1. **Preview performance is acceptable** - We haven't tested with 20+ worktrees
   - Load bearing: If preview is too slow, the whole interface feels sluggish
   - How to validate: Benchmark with varying numbers of worktrees

2. **Status symbols are learnable** - Users will learn what `!+↑⇡` means
   - Load bearing: If symbols are too cryptic, users won't understand worktree state
   - How to validate: User testing with developers unfamiliar with the tool

3. **Fuzzy matching on display text is sufficient** - Users don't need to search by status, age, etc.
   - Load bearing: If users want to filter by metadata, they can't
   - How to validate: Observe how users try to filter/search

4. **Single selection is the right default** - Multi-select isn't needed
   - Load bearing: If users want to batch-remove worktrees, they can't
   - How to validate: Ask users what batch operations they'd want

5. **50% preview size is optimal** - Not too big, not too small
   - Load bearing: If preview is too small, diffs are unreadable; too large, list is cramped
   - How to validate: User preference testing, check other tools' defaults

### Validated Assumptions (Low Risk)

1. **Fuzzy matching is valuable** - Users can find worktrees by typing partial names ✓
2. **Commit message helps identify branches** - Better than just branch name ✓
3. **Preview pane is useful** - Provides essential context for switching decisions ✓
4. **Keyboard-driven is preferred** - Faster than mouse-based selection ✓

## What We Need to Decide

### Critical Decisions

1. **Preview performance strategy**: Pre-compute vs debounce vs simplify vs Rust implementation
2. **Information density**: How much to show in main display vs preview
3. **Color usage**: Monochrome vs minimal color vs full color
4. **Sorting default**: Timestamp vs importance vs user-configurable

### Nice-to-Have Decisions

1. **Multi-action support**: Single-purpose vs multi-action interface
2. **Advanced filtering**: Built-in filters vs query syntax vs none
3. **Multi-select**: Enable or keep single-select
4. **Integration strategy**: Keep `wt select` separate vs merge with `wt list`

## Specific Research Questions

We need external research to help answer:

1. **UX Best Practices**:
   - How do popular fuzzy finders (fzf, telescope.nvim, CtrlP) handle information density?
   - What visual patterns work best for terminal UIs? (colors, alignment, symbols)
   - How do users scan long lists of items in terminal interfaces?

2. **Git Tool Patterns**:
   - How do GitHub CLI (`gh pr list`, `gh repo view`) and GitLab CLI structure their interactive pickers?
   - What does magit (Emacs) show when selecting branches?
   - How does lazygit organize its branch list interface?
   - What information does GitKraken show in its branch list?

3. **Performance Patterns**:
   - How does fzf optimize preview performance with expensive commands?
   - Do any fuzzy finders use caching strategies for previews?
   - What's the standard approach to debouncing in terminal UIs?
   - Are there benchmarks comparing shell-based vs native preview implementations?

4. **Skim-Specific**:
   - What's the complete skim API and feature set?
   - Are there skim-based projects we can learn from? (What do they do well?)
   - What are skim's performance characteristics with large item counts?
   - Can skim previews be implemented in Rust code instead of shell scripts?

5. **User Behavior**:
   - What do developers prioritize when choosing which branch to work on?
   - How often do developers batch-operate on worktrees vs one-at-a-time?
   - What's the typical number of concurrent worktrees developers maintain?
   - Do developers prefer seeing everything at once or filtered views?

## Example Output for Context

Current `wt list` output:
```
Branch    Status  HEAD±      main↕  State         Path
main                                               ./worktrunk
skim      !+      +528  -12                        ./worktrunk.skim
refactor  !        +57 -108                        ./worktrunk.refactor
switch-   ?!       +75   -8                        ./worktrunk.switch-
demo-fix  ↓                     ↓3   (no commits)  ./worktrunk.demo-fix

⚪ Showing 5 worktrees, 3 with changes, 1 behind
```

Current `wt select` main display:
```
main                           Initial commit
skim      !+                   Add skim-based selector
refactor  !                    Refactor list rendering
switch-   ?!                   WIP: switch improvements
demo-fix  ↓                    Fix demo crash
```

Preview pane shows:
```
━━━ Branch: skim ━━━

Working tree:
 M src/commands/select.rs
A  src/commands/select.rs

vs main: ↑2 ↓0

Recent commits:
a1b2c3d Add skim-based selector
d4e5f6g Initial implementation

Changes vs main:
 src/commands/select.rs | 208 +++++++++++++++++++++++
 1 file changed, 208 insertions(+)

Diff preview:
+++ b/src/commands/select.rs
@@ -0,0 +1,208 @@
+use skim::prelude::*;
...
```

## Success Criteria

How will we know we've made `wt select` significantly more useful?

1. **Performance**: Preview feels responsive even with 20+ worktrees (subjective: no perceived lag)
2. **Efficiency**: Users can find and switch to target worktree faster than `wt list` + `wt switch`
3. **Clarity**: Users understand worktree state without reading documentation
4. **Flexibility**: Power users can customize sorting/filtering to match their workflow
5. **Discoverability**: Keyboard shortcuts and capabilities are obvious or easily learned

## Constraints

1. **No external dependencies**: Must use skim (already bundled), can't require fzf or other external tools
2. **Cross-shell compatibility**: Preview scripts must work in bash, zsh, fish, etc. (use `sh -c`)
3. **Terminal compatibility**: Must work in various terminal emulators, respect NO_COLOR, etc.
4. **Performance budget**: Should remain fast even with dozens of worktrees
5. **Maintain consistency**: Should feel like part of the worktrunk ecosystem (similar to `wt list`, `wt switch`)

## Next Steps

We're looking for **IDEAS and RESEARCH** to help us decide:

1. What information should be in the main display vs preview?
2. How to optimize preview performance without sacrificing utility?
3. Should we use colors in the main display? What color scheme?
4. What sorting/filtering options would be most valuable?
5. Should we support multi-action workflows or keep it single-purpose?
6. What can we learn from other fuzzy finders and git tools?

**We are NOT looking for**:
- Implementation details or code (we'll handle that)
- PRs or patches
- Opinions without research backing

**We ARE looking for**:
- Patterns from other tools that work well (with examples)
- UX research on terminal interfaces and information density
- Performance optimization strategies with trade-off analysis
- User behavior insights from git workflows
- Comparative analysis of similar tools (fzf, telescope, lazygit, gh, glab)
- Skim-specific capabilities we're not aware of

---

**Maintainer Note**: This report was generated to seek research assistance. The researcher will only see this document and won't have access to the codebase, so all relevant context has been included above.

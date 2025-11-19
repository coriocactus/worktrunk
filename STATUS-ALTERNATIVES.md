# Status Column: Compact Representations With Same Information

Analysis of how to display the **same information** in the Status column using **fewer characters** through more compact symbols, combined indicators, or overloaded meanings.

**Goal:** Keep Status as a comprehensive summary, reduce space usage from 16 chars while preserving all information.

## Current Status Column Layout (16 chars)

```
Position 0: working_tree    (5 chars) - ?!+»✘
Position 1: conflicts       (1 char)  - =
Position 2: git_operation   (1 char)  - ↻⋈
Position 3: main_divergence (1 char)  - ↑↓↕
Position 4: upstream_div    (1 char)  - ⇡⇣⇅
Position 5: branch_state    (1 char)  - ≡∅
Position 6: worktree_attrs  (3 chars) - ◇⊠⚠
Position 7: user_status     (3 chars) - emoji/labels
                           ─────────
Total: 16 characters
```

**Typical usage:** 0-3 chars, **Maximum possible:** 11 chars (`?!+»✘=⊠↕⇅🤖`)

---

## Strategy 1: Verify Mutual Exclusivity

**Observation:** Some positions contain mutually exclusive symbols (enforced by enums within each field). Need to verify if positions can be combined.

### 1A. Git Operation + Branch State - Can they combine? ❌ NO

**Analysis from code review** (`src/commands/list/collect.rs:99-109, 270-295`):

**Git operation** (↻⋈) = **process state** - ongoing git operation
- `↻` when `.git/rebase-merge` or `.git/rebase-apply` exists
- `⋈` when `.git/MERGE_HEAD` exists

**Branch state** (≡∅) = **content state** - relationship to main
- `≡` when matches main exactly (no commits ahead, working tree matches)
- `∅` when no commits but doesn't match main

**These are orthogonal dimensions** and can occur simultaneously:

**Example scenario:**
```bash
wt switch feature  # At main's HEAD, matches exactly (≡)
git rebase other   # Conflict occurs (↻)
# Result: Both ≡ and ↻ are set
```

**Verdict:** ❌ **Cannot combine** - represent independent dimensions (process vs content state)

---

### 1B. Conflicts + Branch State - Can they combine? ✅ YES

**Analysis from code review** (`src/commands/list/collect.rs:270-295, 420-428`):

**Conflicts** (=) = **would conflict if merged into main**
- Set when `repo.has_merge_conflicts(base, commit_sha)` returns true
- Indicates the worktree has changes that would conflict with main

**Branch state** (≡∅) = **relationship to main**
- `≡` when matches main exactly (no commits ahead, working tree matches)
- `∅` when no commits ahead and clean working tree

**These are mutually exclusive states:**

**Logical analysis:**
```
=  (conflicts)   → Has changes that conflict with main
≡  (matches)     → IS main (identical, no changes)
∅  (no commits)  → Has nothing ahead of main, clean
(none)           → Normal working branch (changes but no conflicts)
```

**Why mutually exclusive:**
- `≡` (matches main): Cannot have conflicts if you ARE main (identical trees)
- `∅` (no commits): Cannot have conflicts if you have nothing ahead of main
- `=` (conflicts): Implies you have changes that differ from main

**Scenarios:**
```bash
# Scenario 1: Matches main (≡)
wt switch feature
# At main's HEAD, no changes
# branch_state = MatchesMain (≡)
# has_conflicts = false (can't conflict if identical)

# Scenario 2: No commits (∅)
wt switch -c empty
# No commits, clean working tree
# branch_state = NoCommits (∅)
# has_conflicts = false (nothing to conflict)

# Scenario 3: Conflicts (=)
# Feature has commits that would conflict with main
# branch_state = None (has commits ahead)
# has_conflicts = true
```

**Verdict:** ✅ **Can combine** - mutually exclusive states that can share one position

**Proposed combined position:**
```
Position: conflicts_or_branch_state (1 char)
  =  ← has merge conflicts with main
  ≡  ← matches main exactly
  ∅  ← no commits, clean
```

**Savings: 1 char** (combines positions 1 and 5)

---

## Strategy 2: Compress Multi-Char Positions

### 2A. Working Tree: Use Combined Symbols (saves 2-3 chars)

**Current:** Up to 5 separate symbols
```
?!+»✘  (5 chars max)
```

**Proposed Option 1: Compound symbols for common combinations**
```
Common patterns as single symbols:
  ⊗  ← ?! (untracked + modified) - most common "dirty" state
  ⊕  ← !+ (modified + staged) - common workflow state
  ?  ← untracked only
  !  ← modified only
  +  ← staged only
  »  ← renamed (rare, stays separate)
  ✘  ← deleted (rare, stays separate)
```

**Analysis:**
- ⚠️ Saves 1 char in common case (`?!` → `⊗`)
- ❌ Requires learning new symbols
- ❌ Doesn't save space when have `?!+` (3 symbols) vs `⊗+` (2 symbols) = saves 1 char only
- ❌ Edge cases with all 5 types still take 4-5 chars

**Verdict:** ⚠️ Marginal savings, added complexity

---

**Proposed Option 2: Single symbol with styling**
```
Use base symbol modified by color/style:
  ∗  ← base "changes" symbol

Variants:
  ∗     CYAN     ← untracked
  ∗     YELLOW   ← modified
  ∗     GREEN    ← staged
  ∗     BOLD     ← has multiple types
```

**Analysis:**
- ✅ Saves 4 chars (5 → 1)
- ❌ **Major information loss** - can't distinguish which types when multiple
- ❌ Color dependency

**Verdict:** ❌ Too much information loss

---

**Proposed Option 3: Abbreviated symbols**
```
?!+»✘  (5 chars) → ?!+RD (5 chars, no savings)
                → ?!+rd (5 chars, no savings)
```

Single-char alternatives that are more compact:
```
?  → .  (dot for untracked)
!  → m  (modified)
+  → s  (staged)
»  → r  (renamed)
✘  → d  (deleted)

Result: ".msr" instead of "?!+»" (same width, less visual)
```

**Analysis:**
- ❌ No space savings
- ❌ Less scannable (. less visible than ?)
- ❌ Letters require reading, not instant recognition

**Verdict:** ❌ No benefit

---

### 2B. Worktree Attributes: Bare is Dead Code! (saves 1 char immediately)

**Current allocation:** 3 chars for `◇⊠⚠`

**Discovery from code review** (`src/git/mod.rs:88`):

```rust
// WorktreeList filters out bare worktrees automatically
let worktrees: Vec<_> = raw_worktrees.into_iter().filter(|wt| !wt.bare).collect();
```

**Analysis:**
- `◇` (bare) is **never shown** - bare worktrees are filtered out before display
- Only `⊠` (locked) and `⚠` (prunable) can actually appear
- The code that renders `◇` is unreachable

**Why bare is filtered:**
- Git worktrees can be "bare" (no working directory)
- This applies to bare repositories themselves
- Worktrunk filters these out because they're not useful to display (can't work in them)
- Only actual worktrees with working directories are shown

**Verdict:** ✅ **Remove bare entirely** - it's dead code

**New maximum:** `⊠⚠` (2 chars: locked + prunable simultaneously)

---

**Proposed Option 1: Keep 2-char allocation**
```
Current max: ⊠⚠  (2 chars - locked + prunable)
Allocation:  2 chars
```

**Analysis:**
- ✅ No information loss
- ✅ Handles the rare case of locked+prunable
- ✅ Simple implementation

**Verdict:** ✅ **Good default** - just reduce allocation from 3→2

---

**Proposed Option 2: Priority symbol only (saves 1 more char)**
```
Priority: ⚠ > ⊠

⚠  ← prunable (with or without locked)
⊠  ← locked only
```

**Analysis:**
- ✅ Saves 1 additional char (2→1)
- ⚠️ Can't distinguish "prunable only" from "prunable + locked"
- ⚠️ How often are both set? If rare, loss is minimal

**Verdict:** ⚠️ Possible if combinations are very rare

---

### 2C. User Status: Reduce Allocation (saves 1 char)

**Current:** 3 chars allocated

**Typical usage:**
- `🤖` (2 chars visual width)
- `💬` (2 chars)
- `WIP` (3 chars)
- `🔥` (2 chars)

**Proposed:** 2 chars allocation
```
Emoji fit fine (most are 2 chars)
Text labels truncated: "WIP" → "WI" or "WP"
```

**Analysis:**
- ✅ Saves 1 char
- ⚠️ Text labels truncated (but emoji are more common)
- ⚠️ Rare 3+ char emoji sequences truncated

**Verdict:** ✅ Reasonable - most users use emoji (2 chars), text truncation acceptable

---

## Strategy 3: Use Color/Style to Overload Meaning

### 3A. Color-Code Conflicts Symbol

**Current:** `=` (red symbol)

**Proposed:** Use color to indicate conflict severity
```
=  RED      ← merge conflicts
=  YELLOW   ← resolved but uncommitted
=  GRAY     ← (unused - conflicts are binary)
```

**Analysis:**
- ❌ Conflicts are binary (exist or don't exist)
- ❌ No additional information to encode
- ❌ No space savings

**Verdict:** ❌ No benefit

---

### 3B. Style Divergence Arrows

**Proposed:** Use arrow style attributes to double-encode information
```
↑  BOLD     ← many commits ahead (>10)
↑  NORMAL   ← few commits ahead
↓  RED      ← many commits behind (>10)
↓  YELLOW   ← few commits behind
```

**Analysis:**
- ⚠️ Numbers are already shown in `main↕` column
- ❌ Duplicates information, doesn't save space
- ❌ Doesn't address the core issue

**Verdict:** ❌ Not applicable

---

## Strategy 4: Reorder Positions to Allow Sharing Space

### 4A. Group Related Symbols to Share Context

**Observation:** Some symbols have implied spacing/positioning

**Current positions:**
```
[working_tree:5] [conflicts:1] [git_op:1] [main_div:1] [upstream_div:1] [branch_state:1] [attrs:3] [user:3]
```

**Proposed:** Reorder to group related items
```
[working_tree:5] [conflicts:1] [git_op:1] [branch_state:1] [main_div:1] [upstream_div:1] [attrs:1+] [user:2]

Where:
- attrs use priority + "more" indicator
- user reduced to 2 chars
```

**Savings calculation:**
- attrs: 3 → 2 (save 1)
- user: 3 → 2 (save 1)
**Total: Save 2 chars**

---

## Summary: Viable Compact Representations

| Change | Saves | Information Loss | Viability |
|--------|-------|------------------|-----------|
| Remove bare (dead code) | 1 char | ✅ None (never shown) | ✅ **Immediate win** |
| Combine conflicts + branch_state | 1 char | ✅ None (mutually exclusive) | ✅ Good |
| Combine git_op + branch_state | - | ❌ Can co-occur (verified) | ❌ No |
| Compound working_tree symbols | 1 char | ⚠️ Learn new symbols | ⚠️ Marginal |
| Abbreviated working_tree | 0 chars | Less scannable | ❌ No |
| Worktree attrs: priority only | 1 char | ⚠️ Lose ⊠⚠ distinction | ⚠️ If rare |
| User status: reduce to 2 chars | 1 char | Text truncation | ✅ Good |

**Conclusion:** Can achieve meaningful compaction through dead code removal and combining mutually exclusive symbols.

---

## Recommended Compact Layout: 13 chars

**Conservative approach with no information loss:**

```
Position 0: working_tree                (5 chars) - ?!+»✘  [keep all]
Position 1: conflicts_or_branch_state   (1 char)  - =≡∅    [combined, mutually exclusive]
Position 2: git_operation               (1 char)  - ↻⋈
Position 3: main_divergence             (1 char)  - ↑↓↕
Position 4: upstream_divergence         (1 char)  - ⇡⇣⇅
Position 5: worktree_attrs              (2 chars) - ⊠⚠    [bare removed - dead code]
Position 6: user_status                 (2 chars) - 🤖    [reduced]
                                       ─────────
Total: 13 chars (saves 3)

Examples:
  ""            ← clean
  "?!+"         ← changes
  "?!+ ≡"       ← changes + matches main
  "= ↻"         ← conflicts + rebasing
  "?!+ = ↻ ↕ ⇅" ← changes + conflicts + rebase + divergences
  "∅        🤖" ← no commits + user status
  "  ⊠⚠  🤖"    ← locked + prunable + user status
```

**Information preserved:**
- ✅ All working tree change types
- ✅ Conflicts (=), matches main (≡), or no commits (∅) - mutually exclusive
- ✅ Git operation (full detail)
- ✅ Main divergence (full detail)
- ✅ Upstream divergence (full detail)
- ✅ Worktree attributes (locked, prunable) - bare removed as dead code
- ✅ User status (emoji fit, text truncated)

**Changes from current (16 chars):**
1. **Conflicts + branch_state: 2 → 1 char** (combined, mutually exclusive)
2. **Worktree attrs: 3 → 2 chars** (removed bare - dead code)
3. **User status: 3 → 2 chars** (reduced allocation)

---

## First Step: Just Remove Bare (15 chars, saves 1)

**Minimal change with zero information loss:**

```
Position 0: working_tree         (5 chars) - ?!+»✘
Position 1: conflicts            (1 char)  - =
Position 2: git_operation        (1 char)  - ↻⋈
Position 3: branch_state         (1 char)  - ≡∅
Position 4: main_divergence      (1 char)  - ↑↓↕
Position 5: upstream_divergence  (1 char)  - ⇡⇣⇅
Position 6: worktree_attrs       (2 chars) - ⊠⚠    [bare removed]
Position 7: user_status          (3 chars) - 🤖
                                ─────────
Total: 15 chars (saves 1)

Changes:
- Remove bare symbol (◇) - it's never rendered (dead code)
- Reduce allocation: 3 → 2 chars for worktree_attrs
```

This is a **safe first step** - removes dead code with zero behavioral change.

---

## Alternative: More Aggressive (12 chars)

**If we accept losing locked+prunable distinction:**

```
Position 0: working_tree                (5 chars) - ?!+»✘
Position 1: conflicts_or_branch_state   (1 char)  - =≡∅
Position 2: git_operation               (1 char)  - ↻⋈
Position 3: main_divergence             (1 char)  - ↑↓↕
Position 4: upstream_divergence         (1 char)  - ⇡⇣⇅
Position 5: worktree_attrs              (1 char)  - ⊠⚠   [priority: ⚠ > ⊠]
Position 6: user_status                 (2 chars) - 🤖
                                       ─────────
Total: 12 chars (saves 4)

Additional loss:
- ⚠️ Can't see when both locked AND prunable (rare edge case)
```

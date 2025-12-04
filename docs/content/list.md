+++
title = "wt list"
weight = 11

[extra]
group = "Commands"
+++

<!-- ⚠️ AUTO-GENERATED from `wt list --help-page` — edit src/cli.rs to update -->

Show all worktrees with their status. The table includes uncommitted changes, divergence from main and remote, and optional CI status.

The table renders progressively: branch names, paths, and commit hashes appear immediately, then status, divergence, and other columns fill in as background git operations complete. CI status (with `--full`) requires network requests and may take longer.

## Examples

List all worktrees:

```bash
wt list
```

Include CI status and conflict detection:

```bash
wt list --full
```

Include branches that don't have worktrees:

```bash
wt list --branches
```

Output as JSON for scripting:

```bash
wt list --format=json
```

## Columns

| Column | Shows |
|--------|-------|
| Branch | Branch name |
| Status | Compact symbols (see below) |
| HEAD± | Uncommitted changes: +added -deleted lines |
| main↕ | Commits ahead/behind main |
| main…± | Line diffs in commits ahead of main (`--full`) |
| Path | Worktree directory |
| Remote⇅ | Commits ahead/behind tracking branch |
| CI | Pipeline status (`--full`) |
| Commit | Short hash (8 chars) |
| Age | Time since last commit |
| Message | Last commit message (truncated) |

The CI column shows GitHub/GitLab pipeline status:

| Indicator | Meaning |
|-----------|---------|
| <span style='color:#0a0'>●</span> green | All checks passed |
| <span style='color:#00a'>●</span> blue | Checks running |
| <span style='color:#a00'>●</span> red | Checks failed |
| <span style='color:#a60'>●</span> yellow | Merge conflicts with base |
| <span style='color:#888'>●</span> gray | No checks configured |
| blank | No PR/MR found |

Any CI dot appears dimmed when there are unpushed local changes (stale status).

## Status Symbols

Symbols appear in the Status column in this order:

| Category | Symbol | Meaning |
|----------|--------|---------|
| Working tree | `+` | Staged files |
| | `!` | Modified files (unstaged) |
| | `?` | Untracked files |
| | `✖` | Merge conflicts |
| | `↻` | Rebase in progress |
| | `⋈` | Merge in progress |
| Branch state | `⊘` | Would conflict if merged to main (`--full` only) |
| | `≡` | Matches main (identical contents) |
| | `_` | No commits (empty branch) |
| Divergence | `↑` | Ahead of main |
| | `↓` | Behind main |
| | `↕` | Diverged from main |
| Remote | `⇡` | Ahead of remote |
| | `⇣` | Behind remote |
| | `⇅` | Diverged from remote |
| Other | `⎇` | Branch without worktree |
| | `⌫` | Prunable (directory missing) |
| | `⊠` | Locked worktree |

Rows are dimmed when the branch has no marginal contribution (`≡` matches main or `_` no commits).

## JSON Output

Query structured data with `--format=json`:

```bash
# Worktrees with conflicts
wt list --format=json | jq '.[] | select(.status.branch_state == "Conflicts")'

# Uncommitted changes
wt list --format=json | jq '.[] | select(.status.working_tree.modified)'

# Current worktree
wt list --format=json | jq '.[] | select(.is_current == true)'

# Branches ahead of main
wt list --format=json | jq '.[] | select(.status.main_divergence == "Ahead")'
```

**Status fields:**
- `working_tree`: `{untracked, modified, staged, renamed, deleted}`
- `branch_state`: `""` | `"Conflicts"` | `"MergeTreeConflicts"` | `"MatchesMain"` | `"NoCommits"`
- `git_operation`: `""` | `"Rebase"` | `"Merge"`
- `main_divergence`: `""` | `"Ahead"` | `"Behind"` | `"Diverged"`
- `upstream_divergence`: `""` | `"Ahead"` | `"Behind"` | `"Diverged"`

**Position fields:**
- `is_main` — Main worktree
- `is_current` — Current directory
- `is_previous` — Previous worktree from [wt switch](@/switch.md)

## See Also

- [wt select](@/select.md) — Interactive worktree picker with live preview

---

## Command Reference

<!-- ⚠️ AUTO-GENERATED from `wt list --help-page` — edit cli.rs to update -->

```
wt list - List worktrees and optionally branches
Usage: wt list [OPTIONS]
       wt list <COMMAND>

Commands:
  statusline  Single-line status for shell prompts

Options:
      --format <FORMAT>
          Output format (table, json)

          [default: table]

      --branches
          Include branches without worktrees

      --remotes
          Include remote branches

      --full
          Show CI, conflicts, diffs

      --progressive
          Show fast info immediately, update with slow info

          Displays local data (branches, paths, status) first, then updates with remote data (CI, upstream) as it arrives. Auto-enabled for TTY.

  -h, --help
          Print help (see a summary with '-h')

Global Options:
  -C <path>
          Working directory for this command

      --config <path>
          User config file path

  -v, --verbose
          Show commands and debug info
```

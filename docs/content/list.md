+++
title = "wt list"
weight = 11

[extra]
group = "Commands"
+++

Show all worktrees with their status. The table includes uncommitted changes, divergence from main and remote, and optional CI status.

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

## Status Symbols

Symbols appear in the Status column in this order:

**Working tree state:**
- `+` — Staged files
- `!` — Modified files (unstaged)
- `?` — Untracked files
- `✖` — Merge conflicts
- `↻` — Rebase in progress
- `⋈` — Merge in progress

**Branch state:**
- `⊘` — Would conflict if merged to main (`--full` only)
- `≡` — Matches main (identical contents)
- `_` — No commits (empty branch)

**Divergence from main:**
- `↑` — Ahead of main
- `↓` — Behind main
- `↕` — Diverged from main

**Remote tracking:**
- `⇡` — Ahead of remote
- `⇣` — Behind remote
- `⇅` — Diverged from remote

**Other:**
- `⎇` — Branch without worktree
- `⚑` — Path mismatch (not at expected location)
- `⌫` — Prunable (directory missing)
- `⊠` — Locked worktree

Rows are dimmed when the branch has no marginal contribution (`≡` matches main or `_` no commits).

## Columns

| Column | Shows |
|--------|-------|
| Branch | Branch name |
| Status | Compact symbols (see above) |
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
- `●` green — All checks passed
- `●` blue — Checks running
- `●` red — Checks failed
- `●` yellow — Merge conflicts with base
- `●` gray — No checks configured
- blank — No PR/MR found
- dimmed — Stale (unpushed local changes)

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
- `is_previous` — Previous worktree from [wt switch](/switch/)

## See Also

- [wt select](/select/) — Interactive worktree picker with live preview

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

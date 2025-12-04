+++
title = "Concepts"
weight = 2
+++

## Why git worktrees?

When working with multiple AI agents (or multiple tasks), you have a few options:

| Approach | Pros | Cons |
|----------|------|------|
| One working tree, many branches | Simple setup | Agents step on each other, can't use git for staging/committing |
| Multiple clones | Full isolation | Slow to set up, drift out of sync |
| Git worktrees | Isolation + shared history | Requires management |

Git worktrees are a built-in git feature that creates multiple working directories from a single repository. Each worktree has its own branch and working tree, while sharing the object database and refs with the main repository. Branches created in one worktree are immediately visible to others with no sync overhead.

For more details, see [git-worktree documentation](https://git-scm.com/docs/git-worktree).

## Why Worktrunk?

Git's `worktree` commands require remembering paths and composing git + `cd` sequences. Worktrunk bundles these into simple commands and adds extended capabilities.

### Simple commands

Worktrunk addresses worktrees by branch name rather than filesystem path, using consistent directory naming (`repo.branch`, customizable):

| Task | Worktrunk | Plain git |
|------|-----------|-----------|
| Switch worktrees | `wt switch feature` | `cd ../repo.feature` |
| Create + start Claude | `wt switch -c -x claude feature` | `git worktree add -b feature ../repo.feature main && cd ../repo.feature && claude` |
| Clean up | `wt remove` | `cd ../repo && git worktree remove ../repo.feature && git branch -d feature` |
| List | `wt list` (with diffstats & status) | `git worktree list` (just names & paths) |

### Extended capabilities

Beyond simplifying common tasks, Worktrunk adds features that git worktrees don't provide.

**[LLM commit messages](@/llm-commits.md)** generate commit messages from diffs using external tools like [llm](https://llm.datasette.io/). Works for regular commits and squash commits during merge.

**[Lifecycle hooks](@/hooks.md)** run project-defined commands at key points: worktree creation, switching, and merging. Use them for dependency installation, dev servers, formatters, tests, or deployment.

**[Unified status](@/list.md)** shows changes, ahead/behind counts, diff stats, and commit messages across all worktrees. With `--full`, it adds CI status (GitHub/GitLab), conflict detection, and line-level diffs against main.

**Safe cleanup** validates that changes are integrated before deleting worktrees and branches.

**[Merge workflow](@/merge.md)** handles the full pipeline: stage, squash, rebase, run hooks, push, and clean up.

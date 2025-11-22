# Worktrunk

<!-- User badges -->

[![Crates.io](https://img.shields.io/crates/v/worktrunk?style=for-the-badge&logo=rust)](https://crates.io/crates/worktrunk)
[![License: MIT](https://img.shields.io/badge/LICENSE-MIT-blue?style=for-the-badge)](https://opensource.org/licenses/MIT)

<!-- Dev badges (uncomment when repo is public and has traction) -->
<!-- [![GitHub CI Status](https://img.shields.io/github/actions/workflow/status/max-sixty/worktrunk/ci.yml?event=push&branch=main&logo=github&style=for-the-badge)](https://github.com/max-sixty/worktrunk/actions?query=branch%3Amain+workflow%3Aci) -->
<!-- [![Downloads](https://img.shields.io/crates/d/worktrunk?style=for-the-badge&logo=rust)](https://crates.io/crates/worktrunk) -->
<!-- [![Stars](https://img.shields.io/github/stars/max-sixty/worktrunk?style=for-the-badge&logo=github)](https://github.com/max-sixty/worktrunk/stargazers) -->

Worktrunk is a CLI tool which makes working with git worktrees much more fluid.
It's designed for those running many concurrent AI coding agents.

For context, git worktrees let multiple agents work on a single repo without
colliding; each agent gets a separate directory with a version of the code. But
creating worktrees, tracking paths & statuses, cleaning up, etc, is manual.
Worktrunk offers control, transparency & automation for this workflow.

## Demo

![Worktrunk Demo](dev/wt-demo/out/wt-demo.gif)

## Quick Start

**Create a worktree:**

<!-- Output from: tests/integration_tests/snapshots/integration__integration_tests__shell_wrapper__tests__readme_example_simple_switch.snap -->

```bash
$ wt switch --create fix-auth
✅ Created new worktree for fix-auth from main at ../repo.fix-auth/
```

...then do work / have an agent do work. Then, when ready...

**Merge it:**

<!-- Output from: tests/snapshots/integration__integration_tests__merge__readme_example_simple.snap -->

```bash
$ wt merge
🔄 Merging 1 commit to main @ a1b2c3d (no commit/squash/rebase needed)

   * a1b2c3d (HEAD -> fix-auth) Implement JWT validation
    auth.rs | 13 +++++++++++++
    1 file changed, 13 insertions(+)
✅ Merged to main (1 commit, 1 file, +13)
🔄 Removing fix-auth worktree & branch in background
```

See [`wt merge`](#wt-merge) for all options.

**List worktrees:**

<!-- Output from: tests/snapshots/integration__integration_tests__list__readme_example_simple_list.snap -->

```bash
$ wt list
Branch     Status  HEAD±  main↕  Path         Remote⇅  Commit    Age            Message
main                             ./test-repo  ↑0 ↓0    b834638e  10 months ago  Initial commit
bugfix-y   ↑              ↑1     ./bugfix-y            412a27c8  10 months ago  Fix bug
feature-x  +       ↑      +5 ↑3  ./feature-x           7fd821aa  10 months ago  Add file 3

⚪ Showing 3 worktrees, 1 with changes, 2 ahead
```

See [`wt list`](#wt-list) for all options.

## Installation

```bash
cargo install worktrunk
wt config shell install  # Sets up shell integration
```

See [Shell Integration](#shell-integration) for details.

## Design Philosophy

Worktrunk is opinionated! It's not designed to be all things to all people. Its
choices optimize for parallel AI agent workflow:

- Trunk-based development
- Lots of short-lived worktrees
- Commits are squashed into linear histories
- Terminal-based coding agents, shell navigation
- Inner dev loops are local

...and that means...

- Total focus on zero cost of additional parallel agents
- A fairly small surface area: three core commands
  - Worktrees are addressed by their branch
- Maximum automation: LLM commit messages, lifecycle hooks, Claude Code hooks
  - A robust "run auto-merge when 'local-CI' passes" approach
- Default is to commit everything and squash marge (but configurable)
- Extreme UI responsiveness; slow ops can't delay fast ones
- Pluggable; adopting Worktrunk for a portion of a workflow doesn't require
  adopting it for everything. standard `git worktree` commands continue working
  fine!

## Automation Features

### LLM Commit Messages

Worktrunk can invoke external commands during merge operations to generate
commit messages. Simon Willison's [llm](https://llm.datasette.io/) tool reads
the diff and a configurable prompt, then returns a formatted commit message.

Add to `~/.config/worktrunk/config.toml`:

```toml
[commit-generation]
command = "llm"
args = ["-m", "claude-haiku-4-5-20251001"]
```

Then `wt merge` will generate commit messages automatically:

<!-- Output from: tests/snapshots/integration__integration_tests__merge__readme_example_complex.snap -->

```bash
$ wt merge
🔄 Squashing 3 commits into 1 (3 files, +33)...
🔄 Generating squash commit message...
  feat(auth): Implement JWT authentication system

  Add comprehensive JWT token handling including validation, refresh logic,
  and authentication tests. This establishes the foundation for secure
  API authentication.

  - Implement token refresh mechanism with expiry handling
  - Add JWT encoding/decoding with signature verification
  - Create test suite covering all authentication flows
✅ Squashed @ a1b2c3d
```

To set up integration: run `wt config --help` to see the setup guide, or `wt config create` to create an example config file.

<details>
<summary>Advanced: Custom Prompt Templates</summary>

Worktrunk uses [minijinja
templates](https://docs.rs/minijinja/latest/minijinja/syntax/index.html) for
commit message prompts. Customize the prompts by setting `template` (inline) or
`template-file` (external file) in the `[commit-generation]` section. Use
`squash-template` / `squash-template-file` for squash commits.

See [`config.example.toml`](dev/config.example.toml) for complete template examples
with all available variables (`git_diff`, `branch`, `recent_commits`, `commits`,
`target_branch`, `repo`).

</details>

### Project Hooks

Automate common tasks by creating `.config/wt.toml` in the repository root. Install dependencies when creating worktrees, start dev servers automatically, run tests before merging.

```toml
# Install deps when creating a worktree
[post-create-command]
"install" = "uv sync"

# Start dev server automatically
[post-start-command]
"dev" = "uv run dev"

# Run tests before merging
[pre-merge-command]
"test" = "uv run pytest"
"lint" = "uv run ruff check"
```

**Example: Creating a worktree with hooks:**

<!-- Output from: tests/snapshots/integration__integration_tests__merge__readme_example_hooks_post_create.snap -->

```bash
$ wt switch --create feature-x
🔄 Running post-create install:
  uv sync
✅ Created new worktree for feature-x from main at ../repo.feature-x/
🔄 Running post-start dev:
  uv run dev

  Resolved 24 packages in 145ms
  Installed 24 packages in 1.2s
```

**Example: Merging with pre-merge hooks:**

<!-- Output from: tests/snapshots/integration__integration_tests__merge__readme_example_hooks_pre_merge.snap -->

```bash
$ wt merge
🔄 Squashing 3 commits into 1 (2 files, +45)...
🔄 Generating squash commit message...
  feat(api): Add user authentication endpoints

  Implement login and token refresh endpoints with JWT validation.
  Includes comprehensive test coverage and input validation.
✅ Squashed @ a1b2c3d
🔄 Running pre-merge test:
  uv run pytest

  ============================= test session starts ==============================
  collected 3 items

  tests/test_auth.py::test_login_success PASSED                            [ 33%]
  tests/test_auth.py::test_login_invalid_password PASSED                   [ 66%]
  tests/test_auth.py::test_token_validation PASSED                         [100%]

  ============================== 3 passed in 0.8s ===============================

🔄 Running pre-merge lint:
  uv run ruff check

  All checks passed!

🔄 Merging 1 commit to main @ a1b2c3d (no rebase needed)
  * a1b2c3d (HEAD -> feature-auth) feat(api): Add user authentication endpoints
   api/auth.py        | 31 +++++++++++++++++++++++++++++++
   tests/test_auth.py | 14 ++++++++++++++
   2 files changed, 45 insertions(+)
✅ Merged to main (1 commit, 2 files, +45)
🔄 Removing feature-auth worktree & branch in background
```

<details>
<summary>All available hooks</summary>

| Hook                    | When It Runs                                                                   | Execution                                     | Failure Behavior             |
| ----------------------- | ------------------------------------------------------------------------------ | --------------------------------------------- | ---------------------------- |
| **post-create-command** | After `git worktree add` completes                                             | Sequential, blocking                          | Logs warning, continues      |
| **post-start-command**  | After post-create completes                                                    | Parallel, non-blocking (background processes) | Logs warning, continues      |
| **pre-commit-command**  | Before committing changes during `wt merge` (both squash and no-squash modes)  | Sequential, blocking, fail-fast               | Terminates merge immediately |
| **pre-merge-command**   | After rebase completes during `wt merge` (validates rebased state before push) | Sequential, blocking, fail-fast               | Terminates merge immediately |
| **post-merge-command**  | After successful merge and push to target branch, before cleanup               | Sequential, blocking                          | Logs warning, continues      |

**Template variables:** `{{ repo }}`, `{{ branch }}`, `{{ worktree }}`, `{{ repo_root }}`, `{{ target }}`

**Skipping hooks:** `wt switch --no-verify` or `wt merge --no-verify`

**Security:** Commands require approval on first run. Use `--force` to bypass.

</details>

### Shell Integration

Worktrunk requires shell integration in order to switch directories, during `wt
switch` & `wt merge`/`wt remove`. To add automatic setup to shell config files
(supports Bash, Zsh, and Fish):

```bash
wt config shell
```

For manual setup instructions, see `wt config shell --help`.

## Tips

**Create an alias for an agent** — Start a new agent-in-worktree in a couple of seconds. For example, to create a worktree and immediately start Claude:

```bash
alias wsl='wt switch --create --execute=claude'
```

Then:

```bash
wsl new-feature
```

...creates a branch, sets up the worktree, runs initialization hooks, and
launches Claude Code in that directory.

**Auto-generate commit messages** — Configure an LLM to generate commit
messages during merge. See [LLM Commit Messages](#llm-commit-messages).

**Automate startup with hooks** — Use `post-create-command` for environment
setup, `post-start-command` for non-blocking tasks. For example, worktrunk uses
`post-start-command` to bootstrap build caches from main via copy-on-write,
eliminating cold compiles (see [worktrunk's config](.config/wt.toml)). See
[Project Hooks](#project-hooks) for details.

**Use `post-merge-command` as a "local CI"** — Running `wt merge` on a worktree
and gating the merge on tests passing is very freeing — `main` is protected from
one agent forgetting to run all tests, without you having to babysit it.

**View Claude Code status from `wt list`** — The Claude Code integration shows
which branches have active sessions in `wt list`. When Claude starts working,
the branch shows `🤖`; hen waiting for input, it shows `💬`. Setup instructions:
[Custom Worktree Status](#custom-worktree-status).

**Delegate to task runners** — Reference existing Taskfile/Justfile/Makefile commands
instead of duplicating logic:

```toml
[post-create-command]
"setup" = "task install"

[pre-merge-command]
"validate" = "just test lint"
```

## All Commands

<details>
<summary><strong><code>wt switch [branch]</code></strong> - Switch to existing worktree or create a new one</summary>

```
Usage: wt switch [OPTIONS] <BRANCH>

Arguments:
  <BRANCH>  Branch, path, '@' (HEAD), '-' (previous), or '^' (main)

Options:
  -c, --create             Create a new branch
  -b, --base <BASE>        Base branch (defaults to default branch)
  -x, --execute <EXECUTE>  Command to run after switch
  -f, --force              Skip approval prompts
      --no-verify          Skip all project hooks
  -h, --help               Print help

Global Options:
  -C <path>                Change working directory
      --config <path>      User config file path
  -v, --verbose            Show commands and debug info
```

## Operation

### Switching to Existing Worktree

- If worktree exists for branch, changes directory via shell integration
- No hooks run
- No branch creation

### Creating New Worktree (`--create`)

1. Creates new branch (defaults to current default branch as base)
2. Creates worktree in configured location (default: `../{{ main_worktree }}.{{ branch }}`)
3. Runs post-create hooks sequentially (blocking)
4. Shows success message
5. Spawns post-start hooks in background (non-blocking)
6. Changes directory to new worktree via shell integration

## Hooks

### post-create (sequential, blocking)

- Run after worktree creation, before success message
- Typically: `npm install`, `cargo build`, setup tasks
- Failures block the operation
- Skip with `--no-verify`

### post-start (parallel, background)

- Spawned after success message shown
- Typically: dev servers, file watchers, editors
- Run in background, failures logged but don't block
- Logs: `.git/wt-logs/{branch}-post-start-{name}.log`
- Skip with `--no-verify`

## Examples

Switch to existing worktree:

```
wt switch feature-branch
```

Create new worktree from main:

```
wt switch --create new-feature
```

Switch to previous worktree:

```
wt switch -
```

Create from specific base:

```
wt switch --create hotfix --base production
```

Create and run command:

```
wt switch --create docs --execute "code ."
```

Skip hooks during creation:

```
wt switch --create temp --no-verify
```

## Shortcuts

Use `@` for current HEAD, `-` for previous, `^` for main:

```
wt switch @                              # Switch to current branch's worktree
wt switch -                              # Switch to previous worktree
wt switch --create new-feature --base=^  # Branch from main (default)
wt switch --create bugfix --base=@       # Branch from current HEAD
wt remove @                              # Remove current worktree
```

</details>

<a id="wt-merge"></a>

<details>
<summary><strong><code>wt merge [target]</code></strong> - Merge, push, and cleanup</summary>

```
Usage: wt merge [OPTIONS] [TARGET]

Arguments:
  [TARGET]  Target branch (defaults to default branch)

Options:
      --no-squash      Skip commit squashing
      --no-commit      Skip commit, squash, and rebase
      --no-remove      Keep worktree after merge
      --no-verify      Skip all project hooks
  -f, --force          Skip approval prompts
      --stage <STAGE>  What to stage before committing [default: all] [possible values: all, tracked, none]
  -h, --help           Print help

Global Options:
  -C <path>            Change working directory
      --config <path>  User config file path
  -v, --verbose        Show commands and debug info
```

## Operation

Commit → Squash → Rebase → Pre-merge hooks → Push → Cleanup → Post-merge hooks

### Commit

Uncommitted changes are staged and committed with LLM message.
Use `--stage=tracked` to stage only tracked files, or `--stage=none` to commit only what's already staged.

### Squash

Multiple commits are squashed into one with LLM message.
Skip with `--no-squash`. Safety backup: `git reflog show refs/wt-backup/<branch>`

### Rebase

Branch is rebased onto target. Conflicts abort the merge immediately.

### Hooks

Pre-merge commands run after rebase (failures abort). Post-merge commands
run after cleanup (failures logged). Skip all with `--no-verify`.

### Push

Fast-forward push to local target branch. Non-fast-forward pushes are rejected.

### Cleanup

Worktree and branch are removed. Skip with `--no-remove`.

## Examples

Basic merge to main:

```
wt merge
```

Merge without squashing:

```
wt merge --no-squash
```

Keep worktree after merging:

```
wt merge --no-remove
```

Skip all hooks:

```
wt merge --no-verify
```

</details>

<details>
<summary><strong><code>wt remove [worktree]</code></strong> - Remove worktree and branch</summary>

```
Usage: wt remove [OPTIONS] [WORKTREES]...

Arguments:
  [WORKTREES]...  Worktree or branch (@ for current)

Options:
      --no-delete-branch  Keep branch after removal
  -D, --force-delete      Delete unmerged branches
      --no-background     Run removal in foreground
  -h, --help              Print help

Global Options:
  -C <path>               Change working directory
      --config <path>     User config file path
  -v, --verbose           Show commands and debug info
```

## Operation

Removes worktree directory, git metadata, and branch. Requires clean working tree.

### No arguments (remove current)

- Removes current worktree and switches to main worktree
- In main worktree: switches to default branch

### By name (remove specific)

- Removes specified worktree(s) and branches
- Current worktree removed last (switches to main first)

### Background removal (default)

- Returns immediately so you can continue working
- Logs: `.git/wt-logs/{branch}-remove.log`
- Use `--no-background` for foreground (blocking)

### Cleanup

Stops any git fsmonitor daemon for the worktree before removal. This prevents orphaned processes when using builtin fsmonitor (`core.fsmonitor=true`). No effect on Watchman users.

## Examples

Remove current worktree and branch:

```
wt remove
```

Remove specific worktree and branch:

```
wt remove feature-branch
```

Remove worktree but keep branch:

```
wt remove --no-delete-branch feature-branch
```

Remove multiple worktrees:

```
wt remove old-feature another-branch
```

Remove in foreground (blocking):

```
wt remove --no-background feature-branch
```

Switch to default in main:

```
wt remove  # (when already in main worktree)
```

</details>

<a id="wt-list"></a>

<details>
<summary><strong><code>wt list</code></strong> - Show all worktrees and branches</summary>

```
Usage: wt list [OPTIONS]

Options:
      --format <FORMAT>  Output format (table, json) [default: table]
      --branches         Include branches without worktrees
      --remotes          Include remote branches
      --full             Show CI, conflicts, diffs
      --progressive      Force progressive (or --no-progressive)
  -h, --help             Print help

Global Options:
  -C <path>              Change working directory
      --config <path>    User config file path
  -v, --verbose          Show commands and debug info
```

## Columns

- **Branch:** Branch name
- **Status:** Quick status symbols (see STATUS SYMBOLS below)
- **HEAD±:** Uncommitted changes vs HEAD (+added -deleted lines, staged + unstaged)
- **main↕:** Commit count ahead↑/behind↓ relative to main (commits in HEAD vs main)
- **main…± (--full):** Line diffs in commits ahead of main (+added -deleted)
- **Path:** Worktree directory location
- **Remote⇅:** Commits ahead↑/behind↓ relative to tracking branch (e.g. origin/branch)
- **CI (--full):** CI pipeline status (tries PR/MR checks first, falls back to branch workflows)
  - ● passed (green) - All checks passed
  - ● running (blue) - Checks in progress
  - ● failed (red) - Checks failed
  - ● conflicts (yellow) - Merge conflicts with base
  - ● no-ci (gray) - PR/MR or workflow found but no checks configured
  - (blank) - No PR/MR or workflow found, or gh/glab CLI unavailable
  - (dimmed) - Stale: unpushed local changes differ from PR/MR head
- **Commit:** Short commit hash (8 chars)
- **Age:** Time since last commit (relative)
- **Message:** Last commit message (truncated)

**STATUS SYMBOLS:**

Order: `?!+»✘ ✖⚠≡∅ ↻⋈ ↑↓↕ ⇡⇣⇅ ⎇⌫⊠`

- `?` Untracked files present
- `!` Modified files (unstaged changes)
- `+` Staged files (ready to commit)
- `»` Renamed files
- `✘` Deleted files
- `✖` **Merge conflicts** - unresolved conflicts in working tree (fix before continuing)
- `⚠` **Would conflict** - merging into main would fail
- `≡` Working tree matches main (identical contents, regardless of commit history)
- `∅` No commits (no commits ahead AND no uncommitted changes)
- `↻` Rebase in progress
- `⋈` Merge in progress
- `↑` Ahead of main branch
- `↓` Behind main branch
- `↕` Diverged (both ahead and behind main)
- `⇡` Ahead of remote tracking branch
- `⇣` Behind remote tracking branch
- `⇅` Diverged (both ahead and behind remote)
- `⎇` Branch indicator (shown for branches without worktrees)
- `⌫` Prunable worktree (directory missing, can be pruned)
- `⊠` Locked worktree (protected from auto-removal)

_Rows are dimmed when no unique work (≡ matches main OR ∅ no commits)._

**JSON OUTPUT:**

Use `--format=json` for structured data. Each object contains two status maps
with the same fields in the same order as STATUS SYMBOLS above:

**`status`** - variant names for querying:

- `working_tree`: `{untracked, modified, staged, renamed, deleted}` booleans
- `branch_state`: `""` | `"Conflicts"` | `"MergeTreeConflicts"` | `"MatchesMain"` | `"NoCommits"`
- `git_operation`: `""` | `"Rebase"` | `"Merge"`
- `main_divergence`: `""` | `"Ahead"` | `"Behind"` | `"Diverged"`
- `upstream_divergence`: `""` | `"Ahead"` | `"Behind"` | `"Diverged"`
- `user_status`: string (optional)

**`status_symbols`** - Unicode symbols for display (same fields, plus `worktree_attrs`: ⎇/⌫/⊠)

Note: `locked` and `prunable` are top-level fields on worktree objects, not in status.

**Query examples:**

```bash
# Find worktrees with conflicts
jq '.[] | select(.status.branch_state == "Conflicts")'

# Find worktrees with untracked files
jq '.[] | select(.status.working_tree.untracked)'

# Find worktrees in rebase or merge
jq '.[] | select(.status.git_operation != "")'

# Get branches ahead of main
jq '.[] | select(.status.main_divergence == "Ahead")'

# Find locked worktrees
jq '.[] | select(.locked != null)'
```

</details>

<details>
<summary><strong><code>wt config</code></strong> - Manage configuration</summary>

```
Usage: wt config [OPTIONS] <COMMAND>

Commands:
  shell          Shell integration setup
  create         Create global configuration file
  list           List configuration files & locations
  refresh-cache  Refresh default branch from remote
  status         Manage branch status markers
  approvals      Manage command approvals

Options:
  -h, --help             Print help

Global Options:
  -C <path>              Change working directory
      --config <path>    User config file path
  -v, --verbose          Show commands and debug info
```

**LLM SETUP GUIDE:**

Enable LLM commit messages

1. Install an LLM tool (llm, aichat)

   ```bash
   uv tool install -U llm
   ```

2. Configure a model

   For Claude:

   ```bash
   llm install llm-anthropic
   llm keys set anthropic
   # Paste your API key from: https://console.anthropic.com/settings/keys
   llm models default claude-sonnet-4-5
   ```

   For OpenAI:

   ```bash
   llm keys set openai
   # Paste your API key from: https://platform.openai.com/api-keys
   ```

3. Test it works

   ```bash
   llm "say hello"
   ```

4. Configure worktrunk

   Add to `~/.config/worktrunk/config.toml`:

   ```toml
   [commit-generation]
   command = "llm"
   ```

Docs: https://llm.datasette.io/ | https://github.com/sigoden/aichat

</details>

<details>
<summary><strong>Beta commands</strong> - Experimental commands for advanced workflows</summary>

These commands are subject to change:

- `wt beta run-hook <hook-type>` - Run a project hook for testing
- `wt beta commit` - Commit changes with LLM message
- `wt beta squash [target]` - Squash commits with LLM message
- `wt beta push [target]` - Push changes to local target branch
- `wt beta rebase [target]` - Rebase onto target
- `wt beta select` - Interactive worktree selector (WIP)

</details>

## Configuration

```bash
wt config list    # Show all config files and locations
wt config create  # Create global config with examples
wt config --help  # Show LLM setup guide
```

<details>
<summary>Configuration details</summary>

**Global config** (`~/.config/worktrunk/config.toml`):

- `worktree-path` - Path template for new worktrees
- `[list]` - Default display options for `wt list` (full, branches, remotes)
- `[commit-generation]` - LLM command and prompt templates
- `[projects."project-id"]` - Per-project approved commands (auto-populated)

**Project config** (`.config/wt.toml` in repository root):

- `[post-create-command]` - Commands after worktree creation
- `[post-start-command]` - Background commands after creation
- `[pre-commit-command]` - Validation before committing
- `[pre-merge-command]` - Validation before merge
- `[post-merge-command]` - Cleanup after merge

---

**Example global config** (`~/.config/worktrunk/config.toml`):

```toml
worktree-path = "../{{ main_worktree }}.{{ branch }}"

[commit-generation]
command = "llm"
args = ["-m", "claude-haiku-4-5-20251001"]

# Approved commands (auto-populated when approving project hooks)
[projects."github.com/user/repo"]
approved-commands = ["npm install", "npm test"]
```

**Example project config** (`.config/wt.toml`): See Project Hooks section above.

**Path template defaults:** `../{{ main_worktree }}.{{ branch }}` (siblings to main repo). Available variables: `{{ main_worktree }}`, `{{ branch }}`, `{{ repo }}`.

</details>

## Advanced Features

### Custom Worktree Status

Add emoji status markers to branches that appear in `wt list`.

```bash
# Set status for current branch
wt config status set "🤖"

# Or use git config directly
git config worktrunk.status.feature-x "💬"
```

**Status appears in the Status column:**

<!-- Output from: tests/snapshots/integration__integration_tests__list__with_user_status.snap -->

```
Branch             Status  HEAD±  main↕  Path                 Remote⇅  Commit    Age            Message
main                                     ./test-repo                   b834638e  10 months ago  Initial commit
clean-no-status    ≡                     ./clean-no-status             b834638e  10 months ago  Initial commit
clean-with-status  ≡ 💬                  ./clean-with-status           b834638e  10 months ago  Initial commit
dirty-no-status     !      +1 -1         ./dirty-no-status             b834638e  10 months ago  Initial commit
dirty-with-status  ≡?🤖                  ./dirty-with-status           b834638e  10 months ago  Initial commit
```

The custom emoji appears directly after the git status symbols.

<details>
<summary>Automation with Claude Code Hooks</summary>

Claude Code can automatically set/clear emoji status when coding sessions start and end. This shows which branches have active AI sessions.

**Easy setup:** The Worktrunk repository includes a `.claude-plugin` directory with pre-configured hooks.

When using Claude:

- Sets status to `🤖` for the current branch when submitting a prompt (working)
- Changes to `💬` when Claude needs input (waiting for permission or idle)
- Clears the status completely when the session ends

**Status from another terminal:**

<!-- Output from: tests/snapshots/integration__integration_tests__list__with_user_status.snap -->

```bash
$ wt list
Branch             Status  HEAD±  main↕  Path                 Remote⇅  Commit    Age            Message
main                                     ./test-repo                   b834638e  10 months ago  Initial commit
clean-no-status    ≡                     ./clean-no-status             b834638e  10 months ago  Initial commit
clean-with-status  ≡ 💬                  ./clean-with-status           b834638e  10 months ago  Initial commit
dirty-no-status     !      +1 -1         ./dirty-no-status             b834638e  10 months ago  Initial commit
dirty-with-status  ≡?🤖                  ./dirty-with-status           b834638e  10 months ago  Initial commit

⚪ Showing 5 worktrees, 1 with changes
```

**How it works:**

- Status is stored as `worktrunk.status.<branch>` in `.git/config`
- Each branch can have its own status emoji
- The hooks automatically detect the current branch and set/clear its status
- Works with any git repository, no special configuration needed

</details>

</details>

### Custom Worktree Paths

By default, worktrees live as siblings to the main repo:

```
myapp/               # main worktree
myapp.feature-x/     # secondary worktree
myapp.bugfix-y/      # secondary worktree
```

Customize the pattern in `~/.config/worktrunk/config.toml`:

```toml
# Inside the repo (keeps everything contained)
worktree-path = ".worktrees/{{ branch }}"

# Shared directory with multiple repos
worktree-path = "../worktrees/{{ main_worktree }}/{{ branch }}"
```

## Status

Worktrunk is in active development. The core features are stable and ready for use. While the project is pre-1.0, the CLI interface and major features are unlikely to change significantly.

<details>
<summary>Developing</summary>

### Releases

Use [cargo-release](https://github.com/crate-ci/cargo-release) to publish new versions:

```bash
cargo install cargo-release

# Bump version, update Cargo.lock, commit, tag, and push
cargo release patch --execute   # 0.1.0 -> 0.1.1
cargo release minor --execute   # 0.1.0 -> 0.2.0
cargo release major --execute   # 0.1.0 -> 1.0.0
```

This updates Cargo.toml and Cargo.lock, creates a commit and tag, then pushes to GitHub. The tag push triggers GitHub Actions to build binaries, create the release, and publish to crates.io.

Run without `--execute` to preview changes first.

</details>

## FAQ

### What commands does Worktrunk execute?

Worktrunk executes commands in three contexts:

1. **Project hooks** (`.config/wt.toml`) - Automation for worktree lifecycle
2. **LLM commands** (`~/.config/worktrunk/config.toml`) - Commit message generation
3. **--execute flag** - Commands provided explicitly

Commands from project hooks and LLM configuration require approval on first run. Approved commands are saved to `~/.config/worktrunk/config.toml` under the project's configuration. If a command changes, worktrunk requires new approval.

**Example approval prompt:**

<!-- Output from: tests/integration_tests/snapshots/integration__integration_tests__approval_pty__approval_prompt_named_commands.snap -->

```
🟡 test-repo needs approval to execute 3 commands:

🔄 post-create install:
  echo 'Installing dependencies...'

🔄 post-create build:
  echo 'Building project...'

🔄 post-create test:
  echo 'Running tests...'

💡 Allow and remember? [y/N]
```

Use `--force` to bypass prompts (useful for CI/automation).

### How does Worktrunk compare to alternatives?

#### vs. Branch Switching

`git checkout` forces all work through a single directory. Switching branches means rebuilding artifacts, restarting dev servers, and stashing changes. Only one branch can be active at a time.

Worktrunk gives each branch its own directory with independent build caches, processes, and editor state. Work on multiple branches simultaneously without rebuilding or stashing.

#### vs. Plain `git worktree`

Git's built-in worktree commands work but require manual lifecycle management:

```bash
# Plain git worktree workflow
git worktree add -b feature-branch ../myapp-feature main
cd ../myapp-feature
# ...work, commit, push...
cd ../myapp
git merge feature-branch
git worktree remove ../myapp-feature
git branch -d feature-branch
```

Worktrunk automates the full lifecycle:

```bash
wt switch --create feature-branch  # Creates worktree, runs setup hooks
# ...work...
wt merge                            # Squashes, merges, removes worktree
```

What `git worktree` doesn't provide:

- Consistent directory naming and cleanup validation
- Project-specific automation (install dependencies, start services)
- Unified status across all worktrees (commits, CI, conflicts, changes)

Worktrunk adds path management, lifecycle hooks, and `wt list --full` for viewing all worktrees—branches, uncommitted changes, commits ahead/behind, CI status, and conflicts—in a single view.

#### vs. git-machete / git-town

Different scopes:

- **git-machete**: Branch stack management in a single directory
- **git-town**: Git workflow automation in a single directory
- **worktrunk**: Multi-worktree management with hooks and status aggregation

These tools can be used together—run git-machete or git-town inside individual worktrees.

#### vs. Git TUIs (lazygit, gh-dash, etc.)

Git TUIs operate on a single repository. Worktrunk manages multiple worktrees, runs automation hooks, and aggregates status across branches (`wt list --full`). Use your preferred TUI inside each worktree directory.

### Installation fails with C compilation errors

If you encounter errors related to tree-sitter or C compilation (like "error: 'for' loop initial declarations are only allowed in C99 mode" or "undefined reference to le16toh"), install without syntax highlighting:

```bash
cargo install worktrunk --no-default-features
```

This disables bash syntax highlighting in command output but keeps all core functionality. The syntax highlighting feature requires C99 compiler support and can fail on older systems or minimal Docker images.

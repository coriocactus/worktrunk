# Worktrunk: Git Worktree Management

A Rust-based CLI tool for managing git worktrees with seamless shell integration.

## Features

- **Shell Integration**: Automatically `cd` to worktrees when switching
- **Multiple Shells**: Supports Bash, Fish, and Zsh
- **Customizable**: Configure command prefix and hook behavior
- **Fast**: Built in Rust for performance
- **Clean Design**: Uses the proven "eval init" pattern from tools like zoxide and starship

## Installation

```bash
cargo build --release
# Copy target/release/wt to a directory in your PATH
```

## Setup

Worktrunk requires a two-step setup for full functionality:

### 1. Shell Integration (for automatic `cd`)

This enables `wt switch` and `wt finish` to automatically change directories.

**Bash** - Add to `~/.bashrc`:
```bash
eval "$(wt init bash)"
```

**Fish** - Add to `~/.config/fish/config.fish`:
```fish
wt init fish | source
```

**Zsh** - Add to `~/.zshrc`:
```bash
eval "$(wt init zsh)"
```

### 2. Shell Completion (for TAB completion)

This enables completion for commands, flags, and branch names.

**Fish:**
```fish
wt completion fish > ~/.config/fish/completions/wt.fish
```

**Bash** - Add to `~/.bashrc` (after the init line):
```bash
eval "$(wt completion bash)"
```

**Zsh** - Add to `~/.zshrc` (after the init line):
```bash
eval "$(wt completion zsh)"
```

### Complete Setup Example (Fish)

```fish
# In ~/.config/fish/config.fish
wt init fish | source

# Run once to install completions
wt completion fish > ~/.config/fish/completions/wt.fish
```

### What Gets Completed

- **Subcommands**: `wt <TAB>` → shows `list`, `switch`, `finish`, `push`, `merge`
- **Flags**: `wt switch --<TAB>` → shows `--create`, `--base`, `--internal`
- **Branch names**: `wt switch <TAB>` → shows branches without worktrees
- **Target branches**: `wt push <TAB>` → shows all branches

**Notes:**
- Zsh currently uses Bash-compatible completion syntax. Dynamic branch completion may require `bashcompinit`. For best results, use Fish or Bash.
- Regenerate completion scripts after updating `wt`
- Debug completion: Set `WT_DEBUG_COMPLETION=1` to see errors

## Usage

### Basic Commands

```bash
# List all worktrees
wt list

# Switch to a worktree (creates if doesn't exist)
wt switch feature-branch

# Finish current worktree and return to primary
wt finish

# Push changes between worktrees
wt push target-worktree

# Merge and cleanup
wt merge main
```

### Customization

**Custom command prefix:**
```bash
# Use a custom prefix instead of 'wt'
eval "$(wt init bash --cmd myprefix)"

# Now use: myprefix switch, myprefix finish, etc.
```

**Enable prompt hook:**
```bash
# Track worktree changes in your prompt
eval "$(wt init bash --hook prompt)"
```

## How It Works

Worktrunk uses a **directive protocol** to communicate with shell wrappers:

1. Shell wrapper calls `wt switch --internal my-branch`
2. Worktrunk outputs special directives mixed with regular output:
   ```
   __WORKTRUNK_CD__/path/to/worktree
   Switched to worktree: my-branch
   ```
3. Shell wrapper parses output, executes `cd` for directives, displays other lines

This separation keeps the Rust binary focused on git logic while letting the shell handle directory changes.

## Development Status

Current implementation:

- ✅ Shell integration infrastructure (eval init pattern)
- ✅ Template-based shell code generation (Askama)
- ✅ Directive protocol (__WORKTRUNK_CD__)
- ✅ Basic CLI structure
- ⏳ Git primitives (coming next)
- ⏳ Worktree operations (coming next)
- ⏳ Advanced features (push, merge, etc.)

See [TODO.md](TODO.md) for detailed roadmap.

## Architecture

```
wt (Rust binary)
├── Core commands (work standalone)
│   ├── wt list
│   ├── wt push
│   └── wt merge
├── Internal commands (for shell wrapper)
│   ├── wt switch --internal → outputs __WORKTRUNK_CD__ directives
│   ├── wt finish --internal → outputs __WORKTRUNK_CD__ directives
│   └── wt hook prompt → for prompt integration
└── Shell integration
    └── wt init <shell> → outputs shell wrapper function
```

## Design Principles

- **Progressive Enhancement**: Works without shell integration, better with it
- **One Canonical Path**: No options, no configuration unless explicitly needed
- **Fast**: Keep shell integration code minimal (<500ms execution time)
- **Stateless**: Binary doesn't maintain state, shell handles environment

## Inspiration

Worktrunk's shell integration pattern is inspired by successful tools:

- **zoxide**: Smarter cd with frequency tracking
- **starship**: Cross-shell prompt customization
- **direnv**: Per-directory environment variables
- **pyenv**: Python version management with shims

## License

MIT (or your preferred license)

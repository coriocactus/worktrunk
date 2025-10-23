# Worktrunk Development Guidelines

> **Note**: This CLAUDE.md is just getting started. More guidelines will be added as patterns emerge.

## Project Status

**This project has no users yet and zero backward compatibility concerns.**

We are in **pre-release development** mode:
- Breaking changes are acceptable and expected
- No migration paths needed for config changes, API changes, or behavior changes
- Optimize for the best solution, not compatibility with previous versions
- Move fast and make bold improvements

When making decisions, prioritize:
1. **Best technical solution** over backward compatibility
2. **Clean design** over maintaining old patterns
3. **Modern conventions** over legacy approaches

Examples of acceptable breaking changes:
- Changing config file locations (e.g., moving from `~/Library/Application Support` to `~/.config`)
- Renaming commands or flags for clarity
- Changing output formats
- Replacing dependencies with better alternatives
- Restructuring the codebase

When the project reaches v1.0 or gains users, we'll adopt stability commitments. Until then, we're free to iterate rapidly.

## CLI Output Formatting Standards

### The anstyle Ecosystem

All styling uses the **anstyle ecosystem** for composable, auto-detecting terminal output:

- **`anstream`**: Auto-detecting I/O streams (println!, eprintln! macros)
- **`anstyle`**: Core styling with inline pattern `{style}text{style:#}`
- **Color detection**: Respects NO_COLOR, CLICOLOR_FORCE, TTY detection

### Message Types

Five canonical message patterns with their emojis:

1. **Progress**: 🔄 + cyan text (operations in progress)
2. **Success**: ✅ + green text (successful completion)
3. **Errors**: ❌ + red text (failures, invalid states)
4. **Warnings**: 🟡 + yellow text (non-blocking issues)
5. **Hints**: 💡 + dimmed text (helpful suggestions)

### Semantic Style Constants

**Style constants defined in `src/styling.rs`:**

- **`ERROR`**: Red (errors, conflicts)
- **`WARNING`**: Yellow (warnings)
- **`HINT`**: Dimmed (hints, secondary information)
- **`CURRENT`**: Magenta + bold (current worktree)
- **`ADDITION`**: Green (diffs, additions)
- **`DELETION`**: Red (diffs, deletions)

**Emoji constants:**

- **`ERROR_EMOJI`**: ❌ (use with ERROR style)
- **`WARNING_EMOJI`**: 🟡 (use with WARNING style)
- **`HINT_EMOJI`**: 💡 (use with HINT style)

### Inline Formatting Pattern

Use anstyle's inline pattern `{style}text{style:#}` where `#` means reset:

```rust
use worktrunk::styling::{eprintln, println, ERROR, ERROR_EMOJI, WARNING, WARNING_EMOJI, HINT, HINT_EMOJI, AnstyleStyle};
use anstyle::{AnsiColor, Color};

// Progress
let cyan = AnstyleStyle::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
println!("🔄 {cyan}Rebasing onto main...{cyan:#}");

// Success
let green = AnstyleStyle::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
println!("✅ {green}Pushed to main{green:#}");

// Error
eprintln!("{ERROR_EMOJI} {ERROR}Working tree has uncommitted changes{ERROR:#}");

// Warning
eprintln!("{WARNING_EMOJI} {WARNING}Uncommitted changes detected{WARNING:#}");

// Hint
println!("{HINT_EMOJI} {HINT}Use 'wt list' to see all worktrees{HINT:#}");
```

### Composing Styles

Compose styles using anstyle methods (`.bold()`, `.fg_color()`, etc.):

```rust
use worktrunk::styling::{eprintln, AnstyleStyle, ERROR, WARNING};

// Error with bold branch name
let error_bold = ERROR.bold();
eprintln!("❌ Branch '{error_bold}{branch}{error_bold:#}' already exists");

// Warning with bold
let warning_bold = WARNING.bold();
eprintln!("🟡 {warning_bold}{message}{warning_bold:#}");

// Just bold (no color)
let bold = AnstyleStyle::new().bold();
println!("Switched to worktree: {bold}{branch}{bold:#}");
```

### Branch Name Formatting

**Always format branch names in bold** when they appear in messages:

```rust
use worktrunk::styling::{AnstyleStyle, ERROR};

// Good - bold branch name in error
let error_bold = ERROR.bold();
eprintln!("❌ Branch '{error_bold}{branch}{error_bold:#}' already exists");

// Good - bold in regular message
let bold = AnstyleStyle::new().bold();
println!("Switched to worktree: {bold}{branch}{bold:#}");

// Bad - plain branch name
println!("Switched to worktree: {branch}")
```

### Information Hierarchy & Path Styling

**Principle: Bold what answers the user's question, dim what provides context.**

Style elements based on **user intent**, not data type. The same information (like a file path) can be primary in one context and secondary in another.

**File paths:**

- **Primary information** (answering the user's main question): **Bold**
  - Example: `wt config list` - paths are the answer to "where is my config?"

- **Secondary information** (contextual metadata): **Dim**
  - Example: `wt switch` output - path provides context, branch name is the answer

```rust
use worktrunk::styling::AnstyleStyle;

// Path as primary answer (config list)
let bold = AnstyleStyle::new().bold();
println!("Global Config: {bold}{}{bold:#}", path.display());

// Path as secondary context (switch output)
let dim = AnstyleStyle::new().dimmed();
println!("✅ Created {bold}{branch}{bold:#}\n  {dim}Path: {}{dim:#}", path.display());
```

**Visual hierarchy patterns:**

| Element | Primary (answers question) | Secondary (provides context) |
|---------|---------------------------|------------------------------|
| Branch names | **Bold** (always) | **Bold** (always) |
| File paths | **Bold** (`config list`) | **Dim** (`switch` output) |
| Config values | Normal | **Dim** |
| Metadata | Dim | **Dim** |

### Color Detection

Colors automatically adjust based on environment:
- Respects `NO_COLOR` (disables)
- Respects `CLICOLOR_FORCE` / `FORCE_COLOR` (enables)
- Auto-detects TTY (colors only on terminals)

All handled automatically by `anstream` macros.

### Design Principles

- **Inline over wrappers** - Use `{style}text{style:#}` pattern, not wrapper functions
- **Composition over special cases** - Use `.bold()`, `.fg_color()`, not `format_X_with_Y()`
- **Semantic constants** - Use `ERROR`, `WARNING`, not raw colors
- **YAGNI for presentation** - Most output needs no styling
- **Minimal output** - Only use formatting where it adds clarity
- **Unicode-aware** - Width calculations respect emoji and CJK characters (via `StyledLine`)
- **Graceful degradation** - Must work without color support

### Complete Examples

```rust
use worktrunk::styling::{
    eprintln, println, AnstyleStyle,
    ERROR, ERROR_EMOJI, WARNING, WARNING_EMOJI, HINT, HINT_EMOJI
};
use anstyle::Style;

// Simple error
eprintln!("{ERROR_EMOJI} {ERROR}Working tree has uncommitted changes{ERROR:#}");

// Error with bold branch name
let error_bold = ERROR.bold();
eprintln!("{ERROR_EMOJI} Branch '{error_bold}{branch}{error_bold:#}' already exists");

// Warning with bold
let warning_bold = WARNING.bold();
eprintln!("{WARNING_EMOJI} {warning_bold}Uncommitted changes detected{warning_bold:#}");

// Hint
println!("{HINT_EMOJI} {HINT}Use 'wt list' to see all worktrees{HINT:#}");

// Bold branch name in regular message
let bold = Style::new().bold();
println!("Switched to worktree: {bold}{branch}{bold:#}");

// Complex multi-part error
let error_bold = ERROR.bold();
eprintln!("{ERROR_EMOJI} Not a fast-forward from '{error_bold}{target_branch}{error_bold:#}' to HEAD");

// Dimmed secondary info
let dim = Style::new().dimmed();
println!("  {dim}Path: {path}{dim:#}");
```

### StyledLine API

For complex table formatting with proper width calculations, use `StyledLine`:

```rust
use worktrunk::styling::StyledLine;
use anstyle::{AnsiColor, Color, Style};

let mut line = StyledLine::new();
let dim = Style::new().dimmed();
let cyan = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));

line.push_styled("Branch", dim);
line.push_raw("  ");
line.push_styled("main", cyan);

println!("{}", line.render());
```

See `src/commands/list/render.rs` for advanced usage.

### Gutter Formatting for Quoted Content

The **gutter** is a subtle visual separator (single space with background color) used for quoted content like commands and configuration.

**Core Principle: Gutter provides all the separation needed**

The gutter's visual distinction is sufficient - no additional indentation required. This keeps the output clean and maximizes horizontal space for content.

#### Formatting Rules

1. **Gutter at column 0**: `format_with_gutter(content)`
   - Gutter appears at column 0
   - Content appears at column 1 (after the gutter + 1 space)
   - The colored background provides visual separation from surrounding text

2. **Preserve internal structure**: Multi-line content maintains its original formatting
   - Don't strip leading whitespace that's part of the content
   - Apply gutter treatment uniformly to each line

#### Examples

**Config display:**
```
Global Config: /path/to/config
 worktree-path = "../{main-worktree}.{branch}"

 [llm]
```

**Command approval:**
```
project wants to execute:
 [ -d {repo_root}/target ] &&
 [ ! -e {worktree}/target ] &&
 cp -cR {repo_root}/target/. {worktree}/target/
```

**Command execution:**
```
🔄 Executing (post-create):
 npm install
```

#### Implementation

```rust
use worktrunk::styling::format_with_gutter;

// Simple usage - gutter provides all separation
print!("{}", format_with_gutter(&command));
print!("{}", format_with_gutter(&config));
```

**Function signature:**
```rust
/// Arguments:
/// - content: Text to format (preserves internal structure for multi-line)
pub fn format_with_gutter(content: &str) -> String
```

## Testing Guidelines

### Testing with --execute Commands

When testing commands that require confirmation (e.g., `wt switch --execute "..."`), use the `--force` flag to skip the interactive prompt:

```bash
# Good - skips confirmation prompt for testing
wt switch --create feature --execute "echo test" --force

# Bad - DO NOT pipe 'yes' to stdin, this crashes Claude
echo yes | wt switch --create feature --execute "echo test"
```

**Why `--force`?**
- Non-interactive testing requires automated approval
- Piping input to stdin interferes with Claude's I/O handling
- `--force` provides explicit, testable behavior

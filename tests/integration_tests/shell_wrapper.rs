//! Shell wrapper integration tests
//!
//! Tests that verify the complete shell integration path - commands executed through
//! the actual shell wrapper (_wt_exec in bash/zsh/fish).
//!
//! These tests ensure that:
//! - Directives are never leaked to users
//! - Output is properly formatted for humans
//! - Shell integration works end-to-end as users experience it

use crate::common::TestRepo;
use insta::assert_snapshot;
use insta_cmd::get_cargo_bin;
use std::fs;
use std::process::Command;
use std::sync::LazyLock;

/// Regex for normalizing temporary directory paths in test snapshots
static TMPDIR_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
        .expect("Invalid tmpdir regex pattern")
});

/// Output from executing a command through a shell wrapper
#[derive(Debug)]
struct ShellOutput {
    /// Combined stdout and stderr as user would see
    combined: String,
    /// Exit code from the command
    exit_code: i32,
}

impl ShellOutput {
    /// Check if output contains no directive leaks
    fn assert_no_directive_leaks(&self) {
        assert!(
            !self.combined.contains("__WORKTRUNK_CD__"),
            "Output contains leaked __WORKTRUNK_CD__ directive:\n{}",
            self.combined
        );
        assert!(
            !self.combined.contains("__WORKTRUNK_EXEC__"),
            "Output contains leaked __WORKTRUNK_EXEC__ directive:\n{}",
            self.combined
        );
    }

    /// Normalize paths in output for snapshot testing
    fn normalized(&self) -> std::borrow::Cow<'_, str> {
        TMPDIR_REGEX.replace_all(&self.combined, "[TMPDIR]")
    }
}

/// Generate a shell wrapper script using the actual `wt init` command
fn generate_wrapper(repo: &TestRepo, shell: &str) -> String {
    let wt_bin = get_cargo_bin("wt");

    let mut cmd = Command::new(&wt_bin);
    cmd.arg("init").arg(shell);

    // Configure environment
    repo.clean_cli_env(&mut cmd);

    let output = cmd
        .output()
        .unwrap_or_else(|_| panic!("Failed to run wt init {}", shell));

    if !output.status.success() {
        panic!(
            "wt init {} failed with exit code: {:?}\nOutput:\n{}",
            shell,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout)
        .unwrap_or_else(|_| panic!("wt init {} produced invalid UTF-8", shell))
}

/// Quote a shell argument if it contains special characters
fn quote_arg(arg: &str) -> String {
    if arg.contains(' ') || arg.contains(';') || arg.contains('\'') {
        format!("'{}'", arg.replace('\'', "'\\''"))
    } else {
        arg.to_string()
    }
}

/// Build a shell script that sources the wrapper and runs a command
fn build_shell_script(shell: &str, repo: &TestRepo, subcommand: &str, args: &[&str]) -> String {
    let wt_bin = get_cargo_bin("wt");
    let wrapper_script = generate_wrapper(repo, shell);
    let mut script = String::new();

    // Set environment variables - syntax varies by shell
    // Don't use 'set -e' in bash/zsh - we want to capture failures and their exit codes.
    // This is tested by test_wrapper_handles_command_failure which verifies
    // that command failures return proper exit codes rather than aborting the script.
    match shell {
        "fish" => {
            script.push_str(&format!("set -x WORKTRUNK_BIN '{}'\n", wt_bin.display()));
            script.push_str(&format!(
                "set -x WORKTRUNK_CONFIG_PATH '{}'\n",
                repo.test_config_path().display()
            ));
            script.push_str("set -x CLICOLOR_FORCE 1\n");
        }
        _ => {
            // bash, zsh
            script.push_str(&format!("export WORKTRUNK_BIN='{}'\n", wt_bin.display()));
            script.push_str(&format!(
                "export WORKTRUNK_CONFIG_PATH='{}'\n",
                repo.test_config_path().display()
            ));
            script.push_str("export CLICOLOR_FORCE=1\n");
        }
    }

    // Source the wrapper
    script.push_str(&wrapper_script);
    script.push('\n');

    // Build the command
    script.push_str("wt ");
    script.push_str(subcommand);
    for arg in args {
        script.push(' ');
        script.push_str(&quote_arg(arg));
    }
    script.push('\n');

    script
}

/// Execute a command through a shell wrapper
///
/// This simulates what actually happens when users run `wt switch`, etc. in their shell:
/// 1. The `wt` function is defined (from shell integration)
/// 2. It calls `_wt_exec --internal switch ...`
/// 3. The wrapper parses NUL-delimited output and handles directives
/// 4. Users see only the final human-friendly output
///
/// Returns ShellOutput with combined output and exit code
fn exec_through_wrapper(
    shell: &str,
    repo: &TestRepo,
    subcommand: &str,
    args: &[&str],
) -> ShellOutput {
    let script = build_shell_script(shell, repo, subcommand, args);

    let mut cmd = Command::new(shell);
    cmd.arg("-c").arg(&script).current_dir(repo.root_path());

    // Configure git environment
    repo.clean_cli_env(&mut cmd);

    let output = cmd
        .output()
        .unwrap_or_else(|_| panic!("Failed to execute {} wrapper", shell));

    // Combine stdout and stderr as users would see in a terminal
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    ShellOutput {
        combined,
        exit_code: output.status.code().unwrap_or(1),
    }
}

mod tests {
    use super::*;
    use rstest::rstest;

    // TODO: Add zsh support
    // Zsh currently uses the bash template (src/shell.rs:313), which includes bash-specific
    // commands like `complete -F` and `COMP_WORDS` that don't exist in zsh.
    // This causes zsh tests to hang waiting for the wrapper to execute.
    // Need to create templates/zsh.zsh with proper zsh syntax:
    // - Use `compdef` instead of `complete` for completions
    // - Use zsh-specific completion system
    // - Fix temp file cleanup (currently tries `\rm` which fails in test environment)

    // ========================================================================
    // Cross-Shell Error Handling Tests
    // ========================================================================
    //
    // These tests use parametrized testing to verify consistent behavior
    // across all supported shells (bash, fish).

    #[rstest]
    #[case("bash")]
    #[case("fish")]
    fn test_wrapper_handles_command_failure(#[case] shell: &str) {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit");

        // Create a worktree that already exists
        repo.add_worktree("existing", "existing");

        // Try to create it again - should fail
        let output = exec_through_wrapper(shell, &repo, "switch", &["--create", "existing"]);

        // Shell-agnostic assertions: these must be true for ALL shells
        assert_eq!(
            output.exit_code, 1,
            "{}: Command should fail with exit code 1",
            shell
        );
        output.assert_no_directive_leaks();
        assert!(
            output.combined.contains("already exists"),
            "{}: Error message should mention 'already exists'.\nOutput:\n{}",
            shell,
            output.combined
        );

        // Snapshot for shell-specific formatting
        assert_snapshot!(
            format!("command_failure_{}", shell),
            output.normalized().as_ref()
        );
    }

    #[rstest]
    #[case("bash")]
    #[case("fish")]
    fn test_wrapper_switch_create(#[case] shell: &str) {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        let output = exec_through_wrapper(shell, &repo, "switch", &["--create", "feature"]);

        // Shell-agnostic assertions
        assert_eq!(output.exit_code, 0, "{}: Command should succeed", shell);
        output.assert_no_directive_leaks();

        assert!(
            output.combined.contains("Created new worktree"),
            "{}: Should show success message",
            shell
        );

        // Shell-specific snapshot
        assert_snapshot!(
            format!("switch_create_{}", shell),
            output.normalized().as_ref()
        );
    }

    #[rstest]
    #[case("bash")]
    #[case("fish")]
    fn test_wrapper_remove(#[case] shell: &str) {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit");

        // Create a worktree to remove
        repo.add_worktree("to-remove", "to-remove");

        let output = exec_through_wrapper(shell, &repo, "remove", &["to-remove"]);

        // Shell-agnostic assertions
        assert_eq!(output.exit_code, 0, "{}: Command should succeed", shell);
        output.assert_no_directive_leaks();

        // Shell-specific snapshot
        assert_snapshot!(format!("remove_{}", shell), output.normalized().as_ref());
    }

    #[rstest]
    #[case("bash")]
    #[case("fish")]
    fn test_wrapper_merge(#[case] shell: &str) {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit");

        // Create a feature branch
        repo.add_worktree("feature", "feature");

        let output = exec_through_wrapper(shell, &repo, "merge", &["main"]);

        // Shell-agnostic assertions
        assert_eq!(output.exit_code, 0, "{}: Command should succeed", shell);
        output.assert_no_directive_leaks();

        // Shell-specific snapshot
        assert_snapshot!(format!("merge_{}", shell), output.normalized().as_ref());
    }

    #[rstest]
    #[case("bash")]
    #[case("fish")]
    fn test_wrapper_switch_with_execute(#[case] shell: &str) {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Use --force to skip approval prompt in tests
        let output = exec_through_wrapper(
            shell,
            &repo,
            "switch",
            &[
                "--create",
                "test-exec",
                "--execute",
                "echo executed",
                "--force",
            ],
        );

        // Shell-agnostic assertions
        assert_eq!(output.exit_code, 0, "{}: Command should succeed", shell);
        output.assert_no_directive_leaks();

        assert!(
            output.combined.contains("executed"),
            "{}: Execute command output missing",
            shell
        );

        // Shell-specific snapshot
        assert_snapshot!(
            format!("switch_with_execute_{}", shell),
            output.normalized().as_ref()
        );
    }

    // ========================================================================
    // Bash Shell Wrapper Tests
    // ========================================================================

    #[test]
    fn test_switch_with_post_start_command_no_directive_leak() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Configure a post-start command in the project config (this is where the bug manifests)
        // The println! in handle_post_start_commands causes directive leaks
        let config_dir = repo.root_path().join(".config");
        fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
        fs::write(
            config_dir.join("wt.toml"),
            r#"post-start-command = "echo 'test command executed'""#,
        )
        .expect("Failed to write project config");

        repo.commit("Add post-start command");

        // Pre-approve the command in user config
        fs::write(
            repo.test_config_path(),
            r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "test-repo"
command = "echo 'test command executed'"
"#,
        )
        .expect("Failed to write user config");

        let output =
            exec_through_wrapper("bash", &repo, "switch", &["--create", "feature-with-hooks"]);

        // The critical assertion: directives must never appear in user-facing output
        // This is where the bug occurs - "🔄 Starting (background):" is printed with println!
        // which causes it to concatenate with the directive
        output.assert_no_directive_leaks();

        assert_eq!(output.exit_code, 0, "Command should succeed");

        // Normalize paths in output for snapshot testing
        // Snapshot the output
        assert_snapshot!(output.normalized().as_ref());
    }

    #[test]
    fn test_switch_with_execute_through_wrapper() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Use --force to skip approval prompt in tests
        let output = exec_through_wrapper(
            "bash",
            &repo,
            "switch",
            &[
                "--create",
                "test-exec",
                "--execute",
                "echo executed",
                "--force",
            ],
        );

        // No directives should leak
        output.assert_no_directive_leaks();

        assert_eq!(output.exit_code, 0, "Command should succeed");

        // The executed command output should appear
        assert!(
            output.combined.contains("executed"),
            "Execute command output missing"
        );

        // Normalize paths in output for snapshot testing
        // Snapshot the output
        assert_snapshot!(output.normalized().as_ref());
    }

    #[test]
    fn test_bash_shell_integration_hint_suppressed() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // When running through the shell wrapper, the "To enable automatic cd" hint
        // should NOT appear because the user already has shell integration
        let output = exec_through_wrapper("bash", &repo, "switch", &["--create", "bash-test"]);

        // Critical: shell integration hint must be suppressed in directive mode
        assert!(
            !output.combined.contains("To enable automatic cd"),
            "Shell integration hint should not appear when running through wrapper. Output:\n{}",
            output.combined
        );

        // Should still have the success message
        assert!(
            output.combined.contains("Created new worktree"),
            "Success message missing"
        );

        assert_snapshot!(output.normalized().as_ref());
    }

    #[test]
    fn test_wrapper_preserves_progress_messages() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Configure a post-start background command that will trigger progress output
        let config_dir = repo.root_path().join(".config");
        fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
        fs::write(
            config_dir.join("wt.toml"),
            r#"post-start-command = "echo 'background task'""#,
        )
        .expect("Failed to write project config");

        repo.commit("Add post-start command");

        // Pre-approve the command in user config
        fs::write(
            repo.test_config_path(),
            r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "test-repo"
command = "echo 'background task'"
"#,
        )
        .expect("Failed to write user config");

        let output = exec_through_wrapper("bash", &repo, "switch", &["--create", "feature-bg"]);

        // No directives should leak
        output.assert_no_directive_leaks();

        assert_eq!(output.exit_code, 0, "Command should succeed");

        // Critical assertion: progress messages should appear to users
        // This is the test that catches the bug where progress() is suppressed in directive mode
        assert!(
            output.combined.contains("Starting (background)"),
            "Progress message 'Starting (background)' missing from output. \
         Output:\n{}",
            output.combined
        );

        // The background command itself should be shown via gutter formatting
        assert!(
            output.combined.contains("background task"),
            "Background command content missing from output"
        );

        // Normalize paths in output for snapshot testing
        // Snapshot the full output
        assert_snapshot!(output.normalized().as_ref());
    }

    // ============================================================================
    // Fish Shell Wrapper Tests
    // ============================================================================
    //
    // These tests verify that the Fish shell wrapper correctly:
    // 1. Parses NUL-delimited directives from `wt --internal`
    // 2. Never leaks directives to users
    // 3. Preserves all user-visible output (progress, success, hints)
    // 4. Handles Fish-specific psub process substitution correctly
    //
    // Fish uses `read -z` to parse NUL-delimited chunks and `psub` for
    // process substitution. These have known limitations (fish-shell #1040)
    // but work correctly for our use case.

    #[test]
    fn test_fish_wrapper_preserves_progress_messages() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Configure a post-start background command that will trigger progress output
        let config_dir = repo.root_path().join(".config");
        fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
        fs::write(
            config_dir.join("wt.toml"),
            r#"post-start-command = "echo 'fish background task'""#,
        )
        .expect("Failed to write project config");

        repo.commit("Add post-start command");

        // Pre-approve the command in user config
        fs::write(
            repo.test_config_path(),
            r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "test-repo"
command = "echo 'fish background task'"
"#,
        )
        .expect("Failed to write user config");

        let output = exec_through_wrapper("fish", &repo, "switch", &["--create", "fish-bg"]);

        // No directives should leak
        output.assert_no_directive_leaks();

        assert_eq!(output.exit_code, 0, "Command should succeed");

        // Critical: progress messages should appear to users through Fish wrapper
        assert!(
            output.combined.contains("Starting (background)"),
            "Progress message 'Starting (background)' missing from Fish wrapper output. \
         Output:\n{}",
            output.combined
        );

        // The background command itself should be shown via gutter formatting
        assert!(
            output.combined.contains("fish background task"),
            "Background command content missing from output"
        );

        assert_snapshot!(output.normalized().as_ref());
    }

    #[test]
    fn test_fish_multiline_command_execution() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Test that Fish wrapper handles multi-line commands correctly
        // This tests Fish's NUL-byte parsing with embedded newlines
        // Use actual newlines in the command string
        let multiline_cmd = "echo 'line 1'; echo 'line 2'; echo 'line 3'";

        // Use --force to skip approval prompt in tests
        let output = exec_through_wrapper(
            "fish",
            &repo,
            "switch",
            &[
                "--create",
                "fish-multiline",
                "--execute",
                multiline_cmd,
                "--force",
            ],
        );

        // No directives should leak
        output.assert_no_directive_leaks();

        assert_eq!(output.exit_code, 0, "Command should succeed");

        // All three lines should be executed and visible
        assert!(output.combined.contains("line 1"), "First line missing");
        assert!(output.combined.contains("line 2"), "Second line missing");
        assert!(output.combined.contains("line 3"), "Third line missing");

        // Normalize paths in output for snapshot testing
        assert_snapshot!(output.normalized().as_ref());
    }

    #[test]
    fn test_fish_wrapper_handles_empty_chunks() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Test edge case: command that produces minimal output
        // This verifies Fish's `test -n "$chunk"` check works correctly
        let output = exec_through_wrapper("fish", &repo, "switch", &["--create", "fish-minimal"]);

        // No directives should leak even with minimal output
        output.assert_no_directive_leaks();

        assert_eq!(output.exit_code, 0, "Command should succeed");

        // Should still show success message
        assert!(
            output.combined.contains("Created new worktree"),
            "Success message missing from minimal output"
        );

        // Normalize paths in output for snapshot testing
        assert_snapshot!(output.normalized().as_ref());
    }
}

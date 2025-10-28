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

/// Generate the bash wrapper script using the actual `wt init` command
fn generate_bash_wrapper(repo: &TestRepo) -> String {
    let wt_bin = get_cargo_bin("wt");

    let mut cmd = Command::new(&wt_bin);
    cmd.arg("init").arg("bash");

    // Configure environment
    repo.clean_cli_env(&mut cmd);

    let output = cmd.output().expect("Failed to run wt init bash");

    if !output.status.success() {
        panic!(
            "wt init bash failed with exit code: {:?}\nOutput:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("wt init bash produced invalid UTF-8")
}

/// Generate the fish wrapper script using the actual `wt init` command
fn generate_fish_wrapper(repo: &TestRepo) -> String {
    let wt_bin = get_cargo_bin("wt");

    let mut cmd = Command::new(&wt_bin);
    cmd.arg("init").arg("fish");

    // Configure environment
    repo.clean_cli_env(&mut cmd);

    let output = cmd.output().expect("Failed to run wt init fish");

    if !output.status.success() {
        panic!(
            "wt init fish failed with exit code: {:?}\nOutput:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("wt init fish produced invalid UTF-8")
}

/// Execute a command through the bash shell wrapper
///
/// This simulates what actually happens when users run `wt switch`, etc. in their shell:
/// 1. The `wt` function is defined (from shell integration)
/// 2. It calls `_wt_exec --internal switch ...`
/// 3. The wrapper parses NUL-delimited output and handles directives
/// 4. Users see only the final human-friendly output
///
/// Returns the combined stdout+stderr output as users would see it
fn exec_through_bash_wrapper(repo: &TestRepo, subcommand: &str, args: &[&str]) -> String {
    let wrapper_script = generate_bash_wrapper(repo);

    // Get the path to wt binary
    let wt_bin = get_cargo_bin("wt");

    // Build the full script that sources the wrapper and runs the command
    let mut script = String::new();
    script.push_str("set -e\n"); // Exit on error
    script.push_str(&format!("export WORKTRUNK_BIN='{}'\n", wt_bin.display()));
    script.push_str(&format!(
        "export WORKTRUNK_CONFIG_PATH='{}'\n",
        repo.test_config_path().display()
    ));
    script.push_str("export CLICOLOR_FORCE=1\n"); // Force colors for snapshot testing

    // Source the wrapper
    script.push_str(&wrapper_script);
    script.push_str("\n");

    // Run the command
    script.push_str("wt ");
    script.push_str(subcommand);
    for arg in args {
        script.push_str(" ");
        // Quote arguments that need special handling (spaces, semicolons, etc.)
        if arg.contains(' ') || arg.contains(';') || arg.contains('\'') {
            script.push_str("'");
            script.push_str(&arg.replace('\'', "'\\''"));
            script.push_str("'");
        } else {
            script.push_str(arg);
        }
    }
    script.push_str("\n");

    // Execute through bash
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(&script).current_dir(repo.root_path());

    // Configure git environment
    repo.clean_cli_env(&mut cmd);

    let output = cmd.output().expect("Failed to execute bash wrapper");

    // Combine stdout and stderr as users would see in a terminal
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    // Check that command succeeded
    if !output.status.success() {
        panic!(
            "Command failed with exit code: {:?}\nOutput:\n{}",
            output.status.code(),
            combined
        );
    }

    combined
}

/// Execute a command through the fish shell wrapper
///
/// This simulates what actually happens when users run `wt switch`, etc. in their Fish shell:
/// 1. The `wt` function is defined (from shell integration)
/// 2. It calls `_wt_exec --internal switch ...`
/// 3. The wrapper parses NUL-delimited output and handles directives
/// 4. Users see only the final human-friendly output
///
/// Returns the combined stdout+stderr output as users would see it
fn exec_through_fish_wrapper(repo: &TestRepo, subcommand: &str, args: &[&str]) -> String {
    let wrapper_script = generate_fish_wrapper(repo);

    // Get the path to wt binary
    let wt_bin = get_cargo_bin("wt");

    // Build the full script that sources the wrapper and runs the command
    let mut script = String::new();

    // Set environment variables
    script.push_str(&format!("set -x WORKTRUNK_BIN '{}'\n", wt_bin.display()));
    script.push_str(&format!(
        "set -x WORKTRUNK_CONFIG_PATH '{}'\n",
        repo.test_config_path().display()
    ));
    script.push_str("set -x CLICOLOR_FORCE 1\n"); // Force colors for snapshot testing

    // Source the wrapper
    script.push_str(&wrapper_script);
    script.push_str("\n");

    // Run the command
    script.push_str("wt ");
    script.push_str(subcommand);
    for arg in args {
        script.push_str(" ");
        // Quote arguments that need special handling (spaces, semicolons, etc.)
        if arg.contains(' ') || arg.contains(';') || arg.contains('\'') {
            script.push_str("'");
            script.push_str(&arg.replace('\'', "'\\''"));
            script.push_str("'");
        } else {
            script.push_str(arg);
        }
    }
    script.push_str("\n");

    // Execute through fish
    let mut cmd = Command::new("fish");
    cmd.arg("-c").arg(&script).current_dir(repo.root_path());

    // Configure git environment
    repo.clean_cli_env(&mut cmd);

    let output = cmd.output().expect("Failed to execute fish wrapper");

    // Combine stdout and stderr as users would see in a terminal
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    // Check that command succeeded
    if !output.status.success() {
        panic!(
            "Command failed with exit code: {:?}\nOutput:\n{}",
            output.status.code(),
            combined
        );
    }

    combined
}

/// Assert that output contains no directive leaks
fn assert_no_directive_leaks(output: &str) {
    assert!(
        !output.contains("__WORKTRUNK_CD__"),
        "Output contains leaked __WORKTRUNK_CD__ directive:\n{}",
        output
    );
    assert!(
        !output.contains("__WORKTRUNK_EXEC__"),
        "Output contains leaked __WORKTRUNK_EXEC__ directive:\n{}",
        output
    );
}

mod tests {
    use super::*;

    // ========================================================================
    // Bash Shell Wrapper Tests
    // ========================================================================

    #[test]
    fn test_switch_create_through_wrapper_no_directive_leak() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        let output = exec_through_bash_wrapper(&repo, "switch", &["--create", "feature"]);

        // The critical assertion: directives must never appear in user-facing output
        assert_no_directive_leaks(&output);

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        // Snapshot the output for regression testing
        assert_snapshot!(normalized.as_ref());
    }

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
            exec_through_bash_wrapper(&repo, "switch", &["--create", "feature-with-hooks"]);

        // The critical assertion: directives must never appear in user-facing output
        // This is where the bug occurs - "🔄 Starting (background):" is printed with println!
        // which causes it to concatenate with the directive
        assert_no_directive_leaks(&output);

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        // Snapshot the output
        assert_snapshot!(normalized.as_ref());
    }

    #[test]
    fn test_switch_with_execute_through_wrapper() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Use --force to skip approval prompt in tests
        let output = exec_through_bash_wrapper(
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
        assert_no_directive_leaks(&output);

        // The executed command output should appear
        assert!(
            output.contains("executed"),
            "Execute command output missing"
        );

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        // Snapshot the output
        assert_snapshot!(normalized.as_ref());
    }

    #[test]
    fn test_remove_through_wrapper_no_directive_leak() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit");

        // Create a worktree to remove
        repo.add_worktree("to-remove", "to-remove");

        let output = exec_through_bash_wrapper(&repo, "remove", &["to-remove"]);

        // No directives should leak
        assert_no_directive_leaks(&output);

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        // Snapshot the output
        assert_snapshot!(normalized.as_ref());
    }

    #[test]
    fn test_merge_through_wrapper_no_directive_leak() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit");

        // Create a feature branch
        repo.add_worktree("feature", "feature");

        let output = exec_through_bash_wrapper(&repo, "merge", &["main"]);

        // No directives should leak
        assert_no_directive_leaks(&output);

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        // Snapshot the output
        assert_snapshot!(normalized.as_ref());
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

        let output = exec_through_bash_wrapper(&repo, "switch", &["--create", "feature-bg"]);

        // No directives should leak
        assert_no_directive_leaks(&output);

        // Critical assertion: progress messages should appear to users
        // This is the test that catches the bug where progress() is suppressed in directive mode
        assert!(
            output.contains("Starting (background)"),
            "Progress message 'Starting (background)' missing from output. \
         Output:\n{}",
            output
        );

        // The background command itself should be shown via gutter formatting
        assert!(
            output.contains("background task"),
            "Background command content missing from output"
        );

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        // Snapshot the full output
        assert_snapshot!(normalized.as_ref());
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
    fn test_fish_switch_create_no_directive_leak() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        let output = exec_through_fish_wrapper(&repo, "switch", &["--create", "fish-feature"]);

        // Critical: directives must never appear in user-facing output
        assert_no_directive_leaks(&output);

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        assert_snapshot!(normalized.as_ref());
    }

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

        let output = exec_through_fish_wrapper(&repo, "switch", &["--create", "fish-bg"]);

        // No directives should leak
        assert_no_directive_leaks(&output);

        // Critical: progress messages should appear to users through Fish wrapper
        assert!(
            output.contains("Starting (background)"),
            "Progress message 'Starting (background)' missing from Fish wrapper output. \
         Output:\n{}",
            output
        );

        // The background command itself should be shown via gutter formatting
        assert!(
            output.contains("fish background task"),
            "Background command content missing from output"
        );

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        assert_snapshot!(normalized.as_ref());
    }

    #[test]
    fn test_fish_remove_no_directive_leak() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit");

        // Create a worktree to remove
        repo.add_worktree("fish-to-remove", "fish-to-remove");

        let output = exec_through_fish_wrapper(&repo, "remove", &["fish-to-remove"]);

        // No directives should leak
        assert_no_directive_leaks(&output);

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        assert_snapshot!(normalized.as_ref());
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
        let output = exec_through_fish_wrapper(
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
        assert_no_directive_leaks(&output);

        // All three lines should be executed and visible
        assert!(output.contains("line 1"), "First line missing");
        assert!(output.contains("line 2"), "Second line missing");
        assert!(output.contains("line 3"), "Third line missing");

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        assert_snapshot!(normalized.as_ref());
    }

    #[test]
    fn test_fish_wrapper_handles_empty_chunks() {
        let repo = TestRepo::new();
        repo.commit("Initial commit");

        // Test edge case: command that produces minimal output
        // This verifies Fish's `test -n "$chunk"` check works correctly
        let output = exec_through_fish_wrapper(&repo, "switch", &["--create", "fish-minimal"]);

        // No directives should leak even with minimal output
        assert_no_directive_leaks(&output);

        // Should still show success message
        assert!(
            output.contains("Created new worktree"),
            "Success message missing from minimal output"
        );

        // Normalize paths in output for snapshot testing
        let normalized = regex::Regex::new(r"/private/var/folders/[^/]+/[^/]+/T/\.tmp[^/]+")
            .unwrap()
            .replace_all(&output, "[TMPDIR]");

        assert_snapshot!(normalized.as_ref());
    }
}

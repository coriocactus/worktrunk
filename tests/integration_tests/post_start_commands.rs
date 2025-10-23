use crate::common::{TestRepo, make_snapshot_cmd, resolve_git_dir, setup_snapshot_settings};
use insta_cmd::assert_cmd_snapshot;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

// Sleep duration constants for background command tests
// These allow time for background processes to complete and write output files

/// Short wait for fast commands (simple echo statements)
const SLEEP_FAST_COMMAND: Duration = Duration::from_millis(500);

/// Standard wait for background commands with sleep/processing
const SLEEP_BACKGROUND_COMMAND: Duration = Duration::from_secs(1);

/// Extended wait for commands that need extra time (e.g., in e2e shell tests)
const SLEEP_EXTENDED: Duration = Duration::from_secs(2);

/// Helper to create snapshot with normalized paths and SHAs
/// If temp_home is provided, sets HOME environment variable to that path
fn snapshot_switch(test_name: &str, repo: &TestRepo, args: &[&str], temp_home: Option<&Path>) {
    let settings = setup_snapshot_settings(repo);
    settings.bind(|| {
        let mut cmd = make_snapshot_cmd(repo, "switch", args, None);
        if let Some(home) = temp_home {
            cmd.env("HOME", home);
        }
        assert_cmd_snapshot!(test_name, cmd);
    });
}

// ============================================================================
// Post-Create Command Tests (sequential, blocking)
// ============================================================================

#[test]
fn test_post_create_no_config() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Switch without project config should work normally
    snapshot_switch(
        "post_create_no_config",
        &repo,
        &["--create", "feature"],
        None,
    );
}

#[test]
fn test_post_create_single_command() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with a single command (string format)
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-create-command = "echo 'Setup complete'""#,
    )
    .expect("Failed to write config");

    repo.commit("Add config");

    // Pre-approve the command by setting up the user config in temp HOME
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'Setup complete'"
"#,
    )
    .expect("Failed to write user config");

    // Command should execute without prompting
    snapshot_switch(
        "post_create_single_command",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );
}

#[test]
fn test_post_create_multiple_commands_array() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with multiple commands (array format)
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-create-command = ["echo 'First'", "echo 'Second'"]"#,
    )
    .expect("Failed to write config");

    repo.commit("Add config with multiple commands");

    // Pre-approve both commands in temp HOME
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'First'"

[[approved-commands]]
project = "main"
command = "echo 'Second'"
"#,
    )
    .expect("Failed to write user config");

    // Both commands should execute sequentially
    snapshot_switch(
        "post_create_multiple_commands_array",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );
}

#[test]
fn test_post_create_named_commands() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with named commands (table format)
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"[post-create-command]
install = "echo 'Installing deps'"
setup = "echo 'Running setup'"
"#,
    )
    .expect("Failed to write config");

    repo.commit("Add config with named commands");

    // Pre-approve both commands in temp HOME
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'Installing deps'"

[[approved-commands]]
project = "main"
command = "echo 'Running setup'"
"#,
    )
    .expect("Failed to write user config");

    // Commands should execute sequentially
    snapshot_switch(
        "post_create_named_commands",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );
}

#[test]
fn test_post_create_failing_command() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with a command that will fail
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-create-command = "exit 1""#,
    )
    .expect("Failed to write config");

    repo.commit("Add config with failing command");

    // Pre-approve the command in temp HOME
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "exit 1"
"#,
    )
    .expect("Failed to write user config");

    // Should show warning but continue (worktree should still be created)
    snapshot_switch(
        "post_create_failing_command",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );
}

#[test]
fn test_post_create_template_expansion() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with template variables
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-create-command = [
    "echo 'Repo: {main-worktree}' > info.txt",
    "echo 'Branch: {branch}' >> info.txt",
    "echo 'Worktree: {worktree}' >> info.txt",
    "echo 'Root: {repo_root}' >> info.txt"
]"#,
    )
    .expect("Failed to write config");

    repo.commit("Add config with templates");

    // Pre-approve all commands in temp HOME
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    let repo_name = "test-repo";
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "test-repo"
command = "echo 'Repo: {main-worktree}' > info.txt"

[[approved-commands]]
project = "test-repo"
command = "echo 'Branch: {branch}' >> info.txt"

[[approved-commands]]
project = "test-repo"
command = "echo 'Worktree: {worktree}' >> info.txt"

[[approved-commands]]
project = "test-repo"
command = "echo 'Root: {repo_root}' >> info.txt"
"#,
    )
    .expect("Failed to write user config");

    // Commands should execute with expanded templates
    snapshot_switch(
        "post_create_template_expansion",
        &repo,
        &["--create", "feature/test"],
        Some(temp_home.path()),
    );

    // Verify template expansion actually worked by checking the output file
    let worktree_path = repo
        .root_path()
        .parent()
        .unwrap()
        .join(format!("{}.feature-test", repo_name));
    let info_file = worktree_path.join("info.txt");

    assert!(
        info_file.exists(),
        "info.txt should have been created in the worktree"
    );

    let contents = fs::read_to_string(&info_file).expect("Failed to read info.txt");

    // Verify that template variables were actually expanded
    assert!(
        contents.contains(&format!("Repo: {}", repo_name)),
        "Should contain expanded repo name, got: {}",
        contents
    );
    assert!(
        contents.contains("Branch: feature-test"),
        "Should contain expanded branch name (sanitized), got: {}",
        contents
    );
}

// ============================================================================
// Post-Start Command Tests (parallel, background)
// ============================================================================

#[test]
fn test_post_start_single_background_command() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with a background command
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-command = "sleep 1 && echo 'Background task done' > background.txt""#,
    )
    .expect("Failed to write config");

    repo.commit("Add background command");

    // Pre-approve the command
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "sleep 1 && echo 'Background task done' > background.txt"
"#,
    )
    .expect("Failed to write user config");

    // Command should spawn in background (wt exits immediately)
    snapshot_switch(
        "post_start_single_background",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );

    // Verify log file was created
    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");
    let git_dir = resolve_git_dir(&worktree_path);
    let log_dir = git_dir.join("wt-logs");
    assert!(log_dir.exists(), "Log directory should be created");

    // Wait for the background command to complete
    thread::sleep(SLEEP_EXTENDED);

    // Verify the background command actually ran
    let output_file = worktree_path.join("background.txt");
    assert!(
        output_file.exists(),
        "Background command should have created output file"
    );
}

#[test]
fn test_post_start_multiple_background_commands() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with multiple background commands (table format)
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"[post-start-command]
task1 = "echo 'Task 1 running' > task1.txt"
task2 = "echo 'Task 2 running' > task2.txt"
"#,
    )
    .expect("Failed to write config");

    repo.commit("Add multiple background commands");

    // Pre-approve both commands
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'Task 1 running' > task1.txt"

[[approved-commands]]
project = "main"
command = "echo 'Task 2 running' > task2.txt"
"#,
    )
    .expect("Failed to write user config");

    // Commands should spawn in parallel
    snapshot_switch(
        "post_start_multiple_background",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );

    // Wait for background commands
    thread::sleep(SLEEP_BACKGROUND_COMMAND);

    // Verify both tasks ran
    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");
    assert!(worktree_path.join("task1.txt").exists());
    assert!(worktree_path.join("task2.txt").exists());
}

#[test]
fn test_both_post_create_and_post_start() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with both command types
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-create-command = "echo 'Setup done' > setup.txt"

[post-start-command]
server = "sleep 0.5 && echo 'Server running' > server.txt"
"#,
    )
    .expect("Failed to write config");

    repo.commit("Add both command types");

    // Pre-approve all commands
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'Setup done' > setup.txt"

[[approved-commands]]
project = "main"
command = "sleep 0.5 && echo 'Server running' > server.txt"
"#,
    )
    .expect("Failed to write user config");

    // Post-create should run first (blocking), then post-start (background)
    snapshot_switch(
        "both_create_and_start",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );

    // Setup file should exist immediately (post-create is blocking)
    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");
    assert!(
        worktree_path.join("setup.txt").exists(),
        "Post-create command should have completed before wt exits"
    );

    // Wait for background command
    thread::sleep(SLEEP_BACKGROUND_COMMAND);

    // Server file should exist after background task completes
    assert!(
        worktree_path.join("server.txt").exists(),
        "Post-start background command should complete"
    );
}

#[test]
fn test_invalid_toml() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create invalid TOML
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        "post-create-command = [invalid syntax\n",
    )
    .expect("Failed to write config");

    repo.commit("Add invalid config");

    // Should continue without executing commands, showing warning
    snapshot_switch("invalid_toml", &repo, &["--create", "feature"], None);
}

// ============================================================================
// Additional Coverage Tests
// ============================================================================

#[test]
fn test_post_start_log_file_captures_output() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create command that writes to both stdout and stderr
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-command = "echo 'stdout output' && echo 'stderr output' >&2""#,
    )
    .expect("Failed to write config");

    repo.commit("Add command with stdout/stderr");

    // Pre-approve the command
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'stdout output' && echo 'stderr output' >&2"
"#,
    )
    .expect("Failed to write user config");

    snapshot_switch(
        "post_start_log_captures_output",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );

    // Give background command time to complete
    thread::sleep(SLEEP_FAST_COMMAND);

    // Find and read the log file
    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");
    let git_dir = resolve_git_dir(&worktree_path);
    let log_dir = git_dir.join("wt-logs");
    assert!(log_dir.exists(), "Log directory should exist");

    // Find the log file
    let log_files: Vec<_> = fs::read_dir(&log_dir)
        .expect("Failed to read log dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("log"))
        .collect();

    assert_eq!(
        log_files.len(),
        1,
        "Should have exactly one log file, found: {:?}",
        log_files
    );

    let log_contents = fs::read_to_string(&log_files[0]).expect("Failed to read log file");

    // Verify both stdout and stderr were captured
    assert!(
        log_contents.contains("stdout output"),
        "Log should contain stdout, got: {}",
        log_contents
    );
    assert!(
        log_contents.contains("stderr output"),
        "Log should contain stderr, got: {}",
        log_contents
    );
}

#[test]
fn test_post_start_invalid_command_handling() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create command with syntax error (missing quote)
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-command = "echo 'unclosed quote""#,
    )
    .expect("Failed to write config");

    repo.commit("Add invalid command");

    // Pre-approve the command
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'unclosed quote"
"#,
    )
    .expect("Failed to write user config");

    // wt should still complete successfully even if background command has errors
    snapshot_switch(
        "post_start_invalid_command",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );

    // Verify worktree was created despite command error
    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");
    assert!(
        worktree_path.exists(),
        "Worktree should be created even if post-start command fails"
    );
}

#[test]
fn test_post_start_multiple_commands_separate_logs() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create multiple background commands with distinct output
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"[post-start-command]
task1 = "echo 'TASK1_OUTPUT'"
task2 = "echo 'TASK2_OUTPUT'"
task3 = "echo 'TASK3_OUTPUT'"
"#,
    )
    .expect("Failed to write config");

    repo.commit("Add three background commands");

    // Pre-approve all commands
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'TASK1_OUTPUT'"

[[approved-commands]]
project = "main"
command = "echo 'TASK2_OUTPUT'"

[[approved-commands]]
project = "main"
command = "echo 'TASK3_OUTPUT'"
"#,
    )
    .expect("Failed to write user config");

    snapshot_switch(
        "post_start_separate_logs",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );

    // Give background commands time to complete
    thread::sleep(SLEEP_FAST_COMMAND);

    // Verify we have 3 separate log files
    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");
    let git_dir = resolve_git_dir(&worktree_path);
    let log_dir = git_dir.join("wt-logs");
    let log_files: Vec<_> = fs::read_dir(&log_dir)
        .expect("Failed to read log dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("log"))
        .collect();

    assert_eq!(
        log_files.len(),
        3,
        "Should have 3 separate log files, found: {}",
        log_files.len()
    );

    // Read all log files and verify no cross-contamination
    let mut found_outputs = vec![false, false, false];
    for entry in log_files {
        let contents = fs::read_to_string(entry.path()).expect("Failed to read log file");
        let count_task1 = contents.matches("TASK1_OUTPUT").count();
        let count_task2 = contents.matches("TASK2_OUTPUT").count();
        let count_task3 = contents.matches("TASK3_OUTPUT").count();

        // Each log should contain exactly one task's output
        let total_outputs = count_task1 + count_task2 + count_task3;
        assert_eq!(
            total_outputs,
            1,
            "Each log should contain exactly one task's output, found {} in {:?}",
            total_outputs,
            entry.path()
        );

        if count_task1 == 1 {
            found_outputs[0] = true;
        }
        if count_task2 == 1 {
            found_outputs[1] = true;
        }
        if count_task3 == 1 {
            found_outputs[2] = true;
        }
    }

    assert!(
        found_outputs.iter().all(|&x| x),
        "Should find output from all three tasks, found: {:?}",
        found_outputs
    );
}

#[test]
fn test_execute_flag_with_post_start_commands() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create post-start command
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-command = "echo 'Background task' > background.txt""#,
    )
    .expect("Failed to write config");

    repo.commit("Add background command");

    // Pre-approve the command
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'Background task' > background.txt"
"#,
    )
    .expect("Failed to write user config");

    // Use --execute flag along with post-start command
    snapshot_switch(
        "execute_with_post_start",
        &repo,
        &[
            "--create",
            "feature",
            "--execute",
            "echo 'Execute flag' > execute.txt",
        ],
        Some(temp_home.path()),
    );

    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");

    // Execute flag file should exist immediately (synchronous)
    assert!(
        worktree_path.join("execute.txt").exists(),
        "Execute command should run synchronously"
    );

    // Wait for background command
    thread::sleep(SLEEP_FAST_COMMAND);

    // Background file should also exist
    assert!(
        worktree_path.join("background.txt").exists(),
        "Post-start command should also run"
    );
}

#[test]
fn test_post_start_complex_shell_commands() {
    let temp_home = TempDir::new().unwrap();
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create command with pipes and redirects
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-command = "echo 'line1\nline2\nline3' | grep line2 > filtered.txt""#,
    )
    .expect("Failed to write config");

    repo.commit("Add complex shell command");

    // Pre-approve the command
    let user_config_dir = temp_home.path().join(".config/worktrunk");
    fs::create_dir_all(&user_config_dir).expect("Failed to create user config dir");
    fs::write(
        user_config_dir.join("config.toml"),
        r#"worktree-path = "../{main-worktree}.{branch}"

[[approved-commands]]
project = "main"
command = "echo 'line1\nline2\nline3' | grep line2 > filtered.txt"
"#,
    )
    .expect("Failed to write user config");

    snapshot_switch(
        "post_start_complex_shell",
        &repo,
        &["--create", "feature"],
        Some(temp_home.path()),
    );

    // Wait for background command
    thread::sleep(SLEEP_FAST_COMMAND);

    // Verify the piped command worked correctly
    let worktree_path = repo.root_path().parent().unwrap().join("test-repo.feature");
    let filtered_file = worktree_path.join("filtered.txt");
    assert!(
        filtered_file.exists(),
        "Complex shell command should create output file"
    );

    let contents = fs::read_to_string(&filtered_file).expect("Failed to read filtered.txt");
    assert!(
        contents.contains("line2"),
        "Should contain filtered output, got: {}",
        contents
    );
    assert!(
        !contents.contains("line1") && !contents.contains("line3"),
        "Should only contain line2, got: {}",
        contents
    );
}

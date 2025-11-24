use crate::common::{TestRepo, set_temp_home_env, setup_home_snapshot_settings, wt_command};
use insta_cmd::assert_cmd_snapshot;
use std::fs;
use tempfile::TempDir;

/// Test `wt config shell install` with --force flag (skips confirmation)
#[test]
fn test_configure_shell_with_yes() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create a fake .zshrc file
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing config\n").unwrap();

    let settings = setup_home_snapshot_settings(&temp_home);
    settings.bind(|| {
        let mut cmd = wt_command();
        repo.clean_cli_env(&mut cmd);
        set_temp_home_env(&mut cmd, temp_home.path());
        cmd.env("SHELL", "/bin/zsh");
        cmd.arg("config")
            .arg("shell")
            .arg("install")
            .arg("--force")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!(cmd, @r#"
        success: true
        exit_code: 0
        ----- stdout -----
        ✅ Added [1mzsh[0m ~/.zshrc
        [107m [0m  [1m[2m[35mif[0m [1m[2m[34mcommand[0m [2m[36m-v[0m wt [2m[36m>[0m/dev/null [2m[33m2[0m>&1; [1m[2m[35mthen[0m [1m[2m[34meval[0m [2m[32m"$([1m[2m[34mcommand[0m wt config shell init zsh)"[0m; [1m[2m[35mfi[0m[0m

        💡 [2mbash ~/.bashrc (not found)[0m
        💡 [2mfish ~/.config/fish/conf.d (not found)[0m
        ✅ Configured 1 shell

        💡 [2mRestart your shell or run: source ~/.zshrc[0m

        ----- stderr -----
        "#);
    });

    // Verify the file was modified
    let content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(content.contains("eval \"$(command wt config shell init zsh)\""));
}

/// Test `wt config shell install` with specific shell
#[test]
fn test_configure_shell_specific_shell() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create a fake .zshrc file
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing config\n").unwrap();

    let settings = setup_home_snapshot_settings(&temp_home);
    settings.bind(|| {
        let mut cmd = wt_command();
        repo.clean_cli_env(&mut cmd);
        set_temp_home_env(&mut cmd, temp_home.path());
        cmd.env("SHELL", "/bin/zsh");
        cmd.arg("config")
            .arg("shell")
            .arg("install")
            .arg("zsh")
            .arg("--force")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!(cmd, @r#"
        success: true
        exit_code: 0
        ----- stdout -----
        ✅ Added [1mzsh[0m ~/.zshrc
        [107m [0m  [1m[2m[35mif[0m [1m[2m[34mcommand[0m [2m[36m-v[0m wt [2m[36m>[0m/dev/null [2m[33m2[0m>&1; [1m[2m[35mthen[0m [1m[2m[34meval[0m [2m[32m"$([1m[2m[34mcommand[0m wt config shell init zsh)"[0m; [1m[2m[35mfi[0m[0m

        ✅ Configured 1 shell

        💡 [2mRestart your shell or run: source ~/.zshrc[0m

        ----- stderr -----
        "#);
    });

    // Verify the file was modified
    let content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(content.contains("eval \"$(command wt config shell init zsh)\""));
}

/// Test `wt config shell install` when line already exists
#[test]
fn test_configure_shell_already_exists() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create a fake .zshrc file with the line already present
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(
        &zshrc_path,
        "# Existing config\nif command -v wt >/dev/null 2>&1; then eval \"$(command wt config shell init zsh)\"; fi\n",
    )
    .unwrap();

    let settings = setup_home_snapshot_settings(&temp_home);
    settings.bind(|| {
        let mut cmd = wt_command();
        repo.clean_cli_env(&mut cmd);
        set_temp_home_env(&mut cmd, temp_home.path());
        cmd.env("SHELL", "/bin/zsh");
        cmd.arg("config")
            .arg("shell")
            .arg("install")
            .arg("zsh")
            .arg("--force")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!(cmd, @r"
        success: true
        exit_code: 0
        ----- stdout -----
        ⚪ Already configured [1mzsh[0m ~/.zshrc
        ✅ All shells already configured

        ----- stderr -----
        ");
    });

    // Verify the file was not modified (no duplicate)
    let content = fs::read_to_string(&zshrc_path).unwrap();
    let count = content.matches("wt config shell init").count();
    assert_eq!(count, 1, "Should only have one wt config shell init line");
}

/// Test `wt config shell install` for Fish (creates new file in conf.d/)
#[test]
fn test_configure_shell_fish() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    let settings = setup_home_snapshot_settings(&temp_home);
    settings.bind(|| {
        let mut cmd = wt_command();
        repo.clean_cli_env(&mut cmd);
        set_temp_home_env(&mut cmd, temp_home.path());
        cmd.env("SHELL", "/bin/fish");
        cmd.arg("config")
            .arg("shell")
            .arg("install")
            .arg("fish")
            .arg("--force")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!(cmd, @r"
        success: true
        exit_code: 0
        ----- stdout -----
        ✅ Created [1mfish[0m ~/.config/fish/conf.d/wt.fish
        [107m [0m  [1m[2m[35mif[0m [1m[2m[34mtype[0m [2m[36m-q[0m wt; [1m[2m[34mcommand[0m wt config shell init fish [2m[36m|[0m [1m[2m[34msource[0m; end[0m

        ✅ Configured 1 shell

        💡 [2mRestart your shell or run: source ~/.config/fish/conf.d/wt.fish[0m

        ----- stderr -----
        ");
    });

    // Verify the fish conf.d file was created
    let fish_config = temp_home.path().join(".config/fish/conf.d/wt.fish");
    assert!(fish_config.exists(), "Fish config file should be created");

    let content = fs::read_to_string(&fish_config).unwrap();
    assert!(
        content.trim() == "if type -q wt; command wt config shell init fish | source; end",
        "Should contain conditional wrapper: {}",
        content
    );
}

/// Test `wt config shell install` when no config files exist
#[test]
fn test_configure_shell_no_files() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    let settings = setup_home_snapshot_settings(&temp_home);
    settings.bind(|| {
        let mut cmd = wt_command();
        repo.clean_cli_env(&mut cmd);
        set_temp_home_env(&mut cmd, temp_home.path());
        cmd.env("SHELL", "/bin/zsh");
        cmd.arg("config")
            .arg("shell")
            .arg("install")
            .arg("--force")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!(cmd, @r"
        success: false
        exit_code: 1
        ----- stdout -----
        💡 [2mbash ~/.bashrc (not found)[0m
        💡 [2mzsh ~/.zshrc (not found)[0m
        💡 [2mfish ~/.config/fish/conf.d (not found)[0m
        ❌ [31mNo shell config files found[0m

        ----- stderr -----
        ");
    });
}

/// Test `wt config shell install` with multiple existing config files
#[test]
fn test_configure_shell_multiple_configs() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create multiple shell config files
    let bash_config_path = temp_home.path().join(".bashrc");
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&bash_config_path, "# Existing bash config\n").unwrap();
    fs::write(&zshrc_path, "# Existing zsh config\n").unwrap();

    let settings = setup_home_snapshot_settings(&temp_home);
    settings.bind(|| {
        let mut cmd = wt_command();
        repo.clean_cli_env(&mut cmd);
        set_temp_home_env(&mut cmd, temp_home.path());
        cmd.env("SHELL", "/bin/zsh");
        cmd.arg("config")
            .arg("shell")
            .arg("install")
            .arg("--force")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!(cmd, @r#"
        success: true
        exit_code: 0
        ----- stdout -----
        ✅ Added [1mbash[0m ~/.bashrc
        [107m [0m  [1m[2m[35mif[0m [1m[2m[34mcommand[0m [2m[36m-v[0m wt [2m[36m>[0m/dev/null [2m[33m2[0m>&1; [1m[2m[35mthen[0m [1m[2m[34meval[0m [2m[32m"$([1m[2m[34mcommand[0m wt config shell init bash)"[0m; [1m[2m[35mfi[0m[0m

        ✅ Added [1mzsh[0m ~/.zshrc
        [107m [0m  [1m[2m[35mif[0m [1m[2m[34mcommand[0m [2m[36m-v[0m wt [2m[36m>[0m/dev/null [2m[33m2[0m>&1; [1m[2m[35mthen[0m [1m[2m[34meval[0m [2m[32m"$([1m[2m[34mcommand[0m wt config shell init zsh)"[0m; [1m[2m[35mfi[0m[0m

        💡 [2mfish ~/.config/fish/conf.d (not found)[0m
        ✅ Configured 2 shells

        💡 [2mRestart your shell or run: source ~/.zshrc[0m

        ----- stderr -----
        "#);
    });

    // Verify both files were modified
    let bash_content = fs::read_to_string(&bash_config_path).unwrap();
    assert!(
        bash_content.contains("eval \"$(command wt config shell init bash)\""),
        "Bash config should be updated"
    );

    let zsh_content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(
        zsh_content.contains("eval \"$(command wt config shell init zsh)\""),
        "Zsh config should be updated"
    );
}

/// Test `wt config shell install` shows both shells needing updates and already configured shells
#[test]
fn test_configure_shell_mixed_states() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create bash config with wt already configured
    let bash_config_path = temp_home.path().join(".bashrc");
    fs::write(
        &bash_config_path,
        "# Existing config\nif command -v wt >/dev/null 2>&1; then eval \"$(command wt config shell init bash)\"; fi\n",
    )
    .unwrap();

    // Create zsh config without wt
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing zsh config\n").unwrap();

    let settings = setup_home_snapshot_settings(&temp_home);
    settings.bind(|| {
        let mut cmd = wt_command();
        repo.clean_cli_env(&mut cmd);
        set_temp_home_env(&mut cmd, temp_home.path());
        cmd.env("SHELL", "/bin/zsh");
        cmd.arg("config")
            .arg("shell")
            .arg("install")
            .arg("--force")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!(cmd, @r#"
        success: true
        exit_code: 0
        ----- stdout -----
        ⚪ Already configured [1mbash[0m ~/.bashrc
        [107m [0m  [1m[2m[35mif[0m [1m[2m[34mcommand[0m [2m[36m-v[0m wt [2m[36m>[0m/dev/null [2m[33m2[0m>&1; [1m[2m[35mthen[0m [1m[2m[34meval[0m [2m[32m"$([1m[2m[34mcommand[0m wt config shell init bash)"[0m; [1m[2m[35mfi[0m[0m

        ✅ Added [1mzsh[0m ~/.zshrc
        [107m [0m  [1m[2m[35mif[0m [1m[2m[34mcommand[0m [2m[36m-v[0m wt [2m[36m>[0m/dev/null [2m[33m2[0m>&1; [1m[2m[35mthen[0m [1m[2m[34meval[0m [2m[32m"$([1m[2m[34mcommand[0m wt config shell init zsh)"[0m; [1m[2m[35mfi[0m[0m

        💡 [2mfish ~/.config/fish/conf.d (not found)[0m
        ✅ Configured 1 shell

        💡 [2mRestart your shell or run: source ~/.zshrc[0m

        ----- stderr -----
        "#);
    });

    // Verify bash was not modified (already configured)
    let bash_content = fs::read_to_string(&bash_config_path).unwrap();
    let bash_wt_count = bash_content.matches("wt config shell init").count();
    assert_eq!(
        bash_wt_count, 1,
        "Bash should still have exactly one wt config shell init line"
    );

    // Verify zsh was modified
    let zsh_content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(
        zsh_content.contains("eval \"$(command wt config shell init zsh)\""),
        "Zsh config should be updated"
    );
}

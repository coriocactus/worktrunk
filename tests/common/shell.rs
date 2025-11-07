use super::{TestRepo, wt_command};
use insta_cmd::get_cargo_bin;
use std::process::Command;

/// Map shell display names to actual binaries.
pub fn get_shell_binary(shell: &str) -> &str {
    match shell {
        "nushell" => "nu",
        "powershell" => "pwsh",
        "oil" => "osh",
        _ => shell,
    }
}

/// Execute a script in the given shell with the repo's isolated environment.
pub fn execute_shell_script(repo: &TestRepo, shell: &str, script: &str) -> String {
    let binary = get_shell_binary(shell);
    let mut cmd = Command::new(binary);
    repo.clean_cli_env(&mut cmd);

    // Prevent user shell config from leaking into tests.
    cmd.env_remove("BASH_ENV");
    cmd.env_remove("ENV");
    cmd.env_remove("ZDOTDIR");
    cmd.env_remove("XONSHRC");
    cmd.env_remove("XDG_CONFIG_HOME");

    match shell {
        "fish" => {
            cmd.arg("--no-config");
        }
        "powershell" | "pwsh" => {
            cmd.arg("-NoProfile");
        }
        "xonsh" => {
            cmd.arg("--no-rc");
        }
        "nushell" | "nu" => {
            cmd.arg("--no-config-file");
        }
        _ => {}
    }

    let output = cmd
        .arg("-c")
        .arg(script)
        .current_dir(repo.root_path())
        .output()
        .unwrap_or_else(|e| panic!("Failed to execute {} script: {}", shell, e));

    if !output.status.success() {
        panic!(
            "Shell script failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("Invalid UTF-8 in output")
}

/// Generate `wt init <shell>` output for the repo.
pub fn generate_init_code(repo: &TestRepo, shell: &str) -> String {
    let mut cmd = wt_command();
    repo.clean_cli_env(&mut cmd);

    let output = cmd
        .args(["init", shell])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to generate init code");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in init code");

    if !output.status.success() && stdout.trim().is_empty() {
        panic!(
            "Failed to generate init code:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    stdout
}

/// Format PATH mutation per shell.
pub fn path_export_syntax(shell: &str, bin_path: &str) -> String {
    match shell {
        "fish" => format!(r#"set -x PATH {} $PATH"#, bin_path),
        "nushell" => format!(r#"$env.PATH = ($env.PATH | prepend "{}")"#, bin_path),
        "powershell" => format!(r#"$env:PATH = "{}:$env:PATH""#, bin_path),
        "elvish" => format!(r#"set E:PATH = {}:$E:PATH"#, bin_path),
        "xonsh" => format!(r#"$PATH.insert(0, "{}")"#, bin_path),
        _ => format!(r#"export PATH="{}:$PATH""#, bin_path),
    }
}

/// Helper that returns the `wt` binary directory for PATH injection.
pub fn wt_bin_dir() -> String {
    get_cargo_bin("wt")
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string()
}

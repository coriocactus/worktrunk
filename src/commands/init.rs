use clap::Command;
use clap_complete::{Shell as CompletionShell, generate};
use worktrunk::shell;

pub fn handle_init(shell_name: &str, cmd_name: &str, cli_cmd: &mut Command) -> Result<(), String> {
    let shell = shell_name.parse::<shell::Shell>()?;

    let init = shell::ShellInit::new(shell, cmd_name.to_string());

    // Generate shell integration code
    let integration_output = init
        .generate()
        .map_err(|e| format!("Failed to generate shell code: {}", e))?;

    println!("{}", integration_output);

    // Generate and append static completions
    println!();
    println!("# Static completions (commands and flags)");

    // Generate completions to a string so we can filter out hidden commands
    let mut completion_output = Vec::new();
    let completion_shell = match shell {
        shell::Shell::Bash => CompletionShell::Bash,
        shell::Shell::Fish => CompletionShell::Fish,
        shell::Shell::Zsh => CompletionShell::Zsh,
        // Oil Shell is POSIX-compatible, use Bash completions
        shell::Shell::Oil => CompletionShell::Bash,
        // Other shells don't have completion support yet
        shell::Shell::Elvish
        | shell::Shell::Nushell
        | shell::Shell::Powershell
        | shell::Shell::Xonsh => {
            eprintln!("Completion not yet supported for {}", shell);
            std::process::exit(1);
        }
    };
    generate(completion_shell, cli_cmd, "wt", &mut completion_output);

    // Filter out lines for hidden commands (completion, complete)
    let completion_str = String::from_utf8_lossy(&completion_output);
    let filtered: Vec<&str> = completion_str
        .lines()
        .filter(|line| {
            // Remove lines that complete the hidden commands
            !(line.contains("\"completion\"")
                || line.contains("\"complete\"")
                || line.contains("-a \"completion\"")
                || line.contains("-a \"complete\""))
        })
        .collect();

    for line in filtered {
        println!("{}", line);
    }

    Ok(())
}

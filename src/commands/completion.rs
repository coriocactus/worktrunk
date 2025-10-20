use clap::Command;
use clap_complete::{Shell as CompletionShell, generate};
use std::io;
use std::path::Path;
use worktrunk::git::{GitError, get_all_branches_in, get_available_branches};

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
}

pub fn handle_completion(shell: Shell, cli_cmd: &mut Command) {
    let completion_shell = match shell {
        Shell::Bash => CompletionShell::Bash,
        Shell::Fish => CompletionShell::Fish,
        Shell::Zsh => CompletionShell::Zsh,
    };
    generate(completion_shell, cli_cmd, "wt", &mut io::stdout());
}

#[derive(Debug, PartialEq)]
enum CompletionContext {
    SwitchBranch,
    PushTarget,
    MergeTarget,
    BaseFlag,
    Unknown,
}

fn parse_completion_context(args: &[String]) -> CompletionContext {
    // args format: ["wt", "switch", "partial"]
    // or: ["wt", "switch", "--create", "new", "--base", "partial"]

    if args.len() < 2 {
        return CompletionContext::Unknown;
    }

    let subcommand = &args[1];

    // Check if the previous argument was a flag that expects a value
    // If so, we're completing that flag's value
    if args.len() >= 3 {
        let prev_arg = &args[args.len() - 2];
        if prev_arg == "--base" || prev_arg == "-b" {
            return CompletionContext::BaseFlag;
        }
    }

    // Otherwise, complete based on the subcommand's positional argument
    match subcommand.as_str() {
        "switch" => CompletionContext::SwitchBranch,
        "push" => CompletionContext::PushTarget,
        "merge" => CompletionContext::MergeTarget,
        _ => CompletionContext::Unknown,
    }
}

fn get_branches_for_completion<F>(get_branches_fn: F) -> Vec<String>
where
    F: FnOnce() -> Result<Vec<String>, GitError>,
{
    get_branches_fn().unwrap_or_else(|e| {
        if std::env::var("WT_DEBUG_COMPLETION").is_ok() {
            eprintln!("completion error: {}", e);
        }
        Vec::new()
    })
}

pub fn handle_complete(args: Vec<String>) -> Result<(), GitError> {
    let context = parse_completion_context(&args);

    match context {
        CompletionContext::SwitchBranch => {
            // Complete with available branches (excluding those with worktrees)
            let branches = get_branches_for_completion(get_available_branches);
            for branch in branches {
                println!("{}", branch);
            }
        }
        CompletionContext::PushTarget
        | CompletionContext::MergeTarget
        | CompletionContext::BaseFlag => {
            // Complete with all branches
            let branches = get_branches_for_completion(|| get_all_branches_in(Path::new(".")));
            for branch in branches {
                println!("{}", branch);
            }
        }
        CompletionContext::Unknown => {
            // No completions
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_completion_context_switch() {
        let args = vec!["wt".to_string(), "switch".to_string(), "feat".to_string()];
        assert_eq!(
            parse_completion_context(&args),
            CompletionContext::SwitchBranch
        );
    }

    #[test]
    fn test_parse_completion_context_push() {
        let args = vec!["wt".to_string(), "push".to_string(), "ma".to_string()];
        assert_eq!(
            parse_completion_context(&args),
            CompletionContext::PushTarget
        );
    }

    #[test]
    fn test_parse_completion_context_merge() {
        let args = vec!["wt".to_string(), "merge".to_string(), "de".to_string()];
        assert_eq!(
            parse_completion_context(&args),
            CompletionContext::MergeTarget
        );
    }

    #[test]
    fn test_parse_completion_context_base_flag() {
        let args = vec![
            "wt".to_string(),
            "switch".to_string(),
            "--create".to_string(),
            "new".to_string(),
            "--base".to_string(),
            "dev".to_string(),
        ];
        assert_eq!(parse_completion_context(&args), CompletionContext::BaseFlag);
    }

    #[test]
    fn test_parse_completion_context_unknown() {
        let args = vec!["wt".to_string()];
        assert_eq!(parse_completion_context(&args), CompletionContext::Unknown);
    }

    #[test]
    fn test_parse_completion_context_base_flag_short() {
        let args = vec![
            "wt".to_string(),
            "switch".to_string(),
            "--create".to_string(),
            "new".to_string(),
            "-b".to_string(),
            "dev".to_string(),
        ];
        assert_eq!(parse_completion_context(&args), CompletionContext::BaseFlag);
    }

    #[test]
    fn test_parse_completion_context_base_at_end() {
        // --base at the end with empty string (what shell sends when completing)
        let args = vec![
            "wt".to_string(),
            "switch".to_string(),
            "--create".to_string(),
            "new".to_string(),
            "--base".to_string(),
            "".to_string(), // Shell sends empty string for cursor position
        ];
        // Should detect BaseFlag context
        assert_eq!(parse_completion_context(&args), CompletionContext::BaseFlag);
    }

    #[test]
    fn test_parse_completion_context_multiple_base_flags() {
        // Multiple --base flags (last one wins)
        let args = vec![
            "wt".to_string(),
            "switch".to_string(),
            "--create".to_string(),
            "new".to_string(),
            "--base".to_string(),
            "main".to_string(),
            "--base".to_string(),
            "develop".to_string(),
        ];
        assert_eq!(parse_completion_context(&args), CompletionContext::BaseFlag);
    }

    #[test]
    fn test_parse_completion_context_empty_args() {
        let args = vec![];
        assert_eq!(parse_completion_context(&args), CompletionContext::Unknown);
    }

    #[test]
    fn test_parse_completion_context_switch_only() {
        // Just "wt switch" with no other args
        let args = vec!["wt".to_string(), "switch".to_string()];
        assert_eq!(
            parse_completion_context(&args),
            CompletionContext::SwitchBranch
        );
    }
}

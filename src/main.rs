use clap::{Parser, Subcommand};
use arbor::config::ArborConfig;
use arbor::git::{GitError, list_worktrees};
use arbor::shell;
use std::process;

#[derive(Parser)]
#[command(name = "arbor")]
#[command(about = "Git worktree management", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate shell integration code
    Init {
        /// Shell to generate code for (bash, fish, zsh)
        shell: String,

        /// Command prefix (default: arbor)
        #[arg(long, default_value = "arbor")]
        cmd: String,

        /// Hook mode (none, prompt)
        #[arg(long, default_value = "none")]
        hook: String,
    },

    /// List all worktrees
    List,

    /// Switch to a worktree (creates if doesn't exist)
    Switch {
        /// Branch name or worktree path
        branch: String,

        /// Use internal mode (outputs directives for shell wrapper)
        #[arg(long, hide = true)]
        internal: bool,
    },

    /// Finish current worktree and return to primary
    Finish {
        /// Use internal mode (outputs directives for shell wrapper)
        #[arg(long, hide = true)]
        internal: bool,
    },

    /// Push changes between worktrees
    Push {
        /// Target worktree
        target: String,
    },

    /// Merge and cleanup worktree
    Merge {
        /// Target branch to merge into
        target: String,

        /// Squash commits
        #[arg(long)]
        squash: bool,
    },

    /// Hook commands (for shell integration)
    Hook {
        /// Hook type
        hook_type: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { shell, cmd, hook } => {
            handle_init(&shell, &cmd, &hook).map_err(GitError::CommandFailed)
        }
        Commands::List => handle_list(),
        Commands::Switch { branch, internal } => {
            handle_switch(&branch, internal).map_err(GitError::CommandFailed)
        }
        Commands::Finish { internal } => handle_finish(internal).map_err(GitError::CommandFailed),
        Commands::Push { target } => handle_push(&target).map_err(GitError::CommandFailed),
        Commands::Merge { target, squash } => {
            handle_merge(&target, squash).map_err(GitError::CommandFailed)
        }
        Commands::Hook { hook_type } => handle_hook(&hook_type).map_err(GitError::CommandFailed),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn handle_init(shell_name: &str, cmd: &str, hook_str: &str) -> Result<(), String> {
    let shell = shell::Shell::from_str(shell_name)?;
    let hook = shell::Hook::from_str(hook_str)?;

    let init = shell::ShellInit::new(shell, cmd.to_string(), hook);

    let output = init
        .generate()
        .map_err(|e| format!("Failed to generate shell code: {}", e))?;

    println!("{}", output);
    Ok(())
}

fn handle_list() -> Result<(), GitError> {
    let worktrees = list_worktrees()?;

    for wt in worktrees {
        println!("{}", wt.path.display());
        println!("  HEAD: {}", &wt.head[..8.min(wt.head.len())]);

        if let Some(branch) = wt.branch {
            println!("  branch: {}", branch);
        }

        if wt.detached {
            println!("  (detached)");
        }

        if wt.bare {
            println!("  (bare)");
        }

        if let Some(reason) = wt.locked {
            if reason.is_empty() {
                println!("  (locked)");
            } else {
                println!("  (locked: {})", reason);
            }
        }

        if let Some(reason) = wt.prunable {
            if reason.is_empty() {
                println!("  (prunable)");
            } else {
                println!("  (prunable: {})", reason);
            }
        }

        println!();
    }

    Ok(())
}

fn handle_switch(branch: &str, internal: bool) -> Result<(), String> {
    if internal {
        // Internal mode: output directives
        // TODO: Implement actual worktree switching logic
        println!("__ARBOR_CD__/tmp/example-worktree");
        println!("Switched to worktree: {}", branch);
    } else {
        println!("Switching to worktree: {}", branch);
        println!("Note: Use 'arbor-switch' (with shell integration) for automatic cd");
    }
    Ok(())
}

fn handle_finish(internal: bool) -> Result<(), String> {
    if internal {
        // Internal mode: output directives
        // TODO: Implement actual finish logic
        println!("__ARBOR_CD__/tmp/main-worktree");
        println!("Finished worktree and returned to primary");
    } else {
        println!("Finishing worktree");
        println!("Note: Use 'arbor-finish' (with shell integration) for automatic cd");
    }
    Ok(())
}

fn handle_push(target: &str) -> Result<(), String> {
    // TODO: Implement actual push logic
    println!("Pushing to worktree: {}", target);
    Ok(())
}

fn handle_merge(target: &str, squash: bool) -> Result<(), String> {
    // TODO: Implement actual merge logic
    println!("Merging into: {} (squash: {})", target, squash);
    Ok(())
}

fn handle_hook(hook_type: &str) -> Result<(), String> {
    match hook_type {
        "prompt" => {
            // TODO: Implement prompt hook logic
            // This would update tracking, show current worktree, etc.
            Ok(())
        }
        _ => Err(format!("Unknown hook type: {}", hook_type)),
    }
}

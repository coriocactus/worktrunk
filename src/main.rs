use clap::{CommandFactory, Parser, Subcommand};
use std::process;
use worktrunk::config::load_config;
use worktrunk::git::GitError;

mod commands;
mod display;
mod llm;

use commands::{
    Shell, handle_complete, handle_completion, handle_init, handle_list, handle_merge, handle_push,
    handle_remove, handle_switch,
};

#[derive(Parser)]
#[command(name = "wt")]
#[command(about = "Git worktree management", long_about = None)]
#[command(version = env!("VERGEN_GIT_DESCRIBE"))]
#[command(disable_help_subcommand = true)]
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

        /// Command prefix (default: wt)
        #[arg(long, default_value = "wt")]
        cmd: String,
    },

    /// List all worktrees
    List,

    /// Switch to a worktree
    Switch {
        /// Branch name or worktree path
        branch: String,

        /// Create a new branch
        #[arg(short = 'c', long)]
        create: bool,

        /// Base branch to create from (only with --create)
        #[arg(short = 'b', long)]
        base: Option<String>,

        /// Use internal mode (outputs directives for shell wrapper)
        #[arg(long, hide = true)]
        internal: bool,
    },

    /// Finish current worktree, returning to primary if current
    Remove {
        /// Use internal mode (outputs directives for shell wrapper)
        #[arg(long, hide = true)]
        internal: bool,
    },

    /// Push changes between worktrees
    Push {
        /// Target branch (defaults to default branch)
        target: Option<String>,

        /// Allow pushing merge commits (non-linear history)
        #[arg(long)]
        allow_merge_commits: bool,
    },

    /// Merge worktree into target branch
    Merge {
        /// Target branch to merge into (defaults to default branch)
        target: Option<String>,

        /// Squash all commits into one before merging
        #[arg(short, long)]
        squash: bool,

        /// Keep worktree after merging (don't remove)
        #[arg(short, long)]
        keep: bool,
    },

    /// Generate shell completion script (deprecated - use init instead)
    #[command(hide = true)]
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Internal completion helper (hidden)
    #[command(hide = true)]
    Complete {
        /// Arguments to complete
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { shell, cmd } => {
            let mut cli_cmd = Cli::command();
            handle_init(&shell, &cmd, &mut cli_cmd).map_err(GitError::CommandFailed)
        }
        Commands::List => handle_list(),
        Commands::Switch {
            branch,
            create,
            base,
            internal,
        } => load_config()
            .map_err(|e| GitError::CommandFailed(format!("Failed to load config: {}", e)))
            .and_then(|config| {
                handle_switch(
                    &branch,
                    create,
                    base.as_deref(),
                    internal,
                    &config.worktree_path,
                )
            }),
        Commands::Remove { internal } => handle_remove(internal),
        Commands::Push {
            target,
            allow_merge_commits,
        } => handle_push(target.as_deref(), allow_merge_commits),
        Commands::Merge {
            target,
            squash,
            keep,
        } => handle_merge(target.as_deref(), squash, keep),
        Commands::Completion { shell } => {
            let mut cli_cmd = Cli::command();
            handle_completion(shell, &mut cli_cmd);
            Ok(())
        }
        Commands::Complete { args } => handle_complete(args),
    };

    if let Err(e) = result {
        // Error messages are already formatted with emoji and colors
        eprintln!("{}", e);
        process::exit(1);
    }
}

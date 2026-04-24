use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "git-ca", version, about = "Draft git commit messages with GitHub Copilot", disable_help_subcommand = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Skip pre-commit and commit-msg hooks.
    #[arg(short = 'n', long = "no-verify", global = true)]
    pub no_verify: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage GitHub Copilot authentication.
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// List Copilot chat models available to your account.
    Models,
    /// Read or change persisted config (e.g. default model).
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthAction {
    /// Log in via GitHub device flow.
    Login,
    /// Forget stored tokens.
    Logout,
    /// Show auth state and Copilot token TTL.
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Set the default model used when `--model` is not passed.
    SetModel { id: String },
    /// Print the default model (if any).
    GetModel,
}

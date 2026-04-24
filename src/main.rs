mod auth;
mod cli;
mod config;
mod error;

use clap::Parser;
use cli::{AuthAction, Cli, Command, ConfigAction};
use error::{Error, Result};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match run(cli).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("git-ca: {e}");
            e.exit_code()
        }
    };
    std::process::exit(code);
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => commit(cli.no_verify).await,
        Some(Command::Auth { action }) => match action {
            AuthAction::Login => auth_login().await,
            AuthAction::Logout => auth_logout().await,
            AuthAction::Status => auth_status().await,
        },
        Some(Command::Models) => models().await,
        Some(Command::Config { action }) => match action {
            ConfigAction::SetModel { id } => config_set_model(&id).await,
            ConfigAction::GetModel => config_get_model().await,
        },
    }
}

async fn commit(_no_verify: bool) -> Result<()> {
    Err(Error::Config("commit flow not implemented yet".into()))
}
async fn auth_login() -> Result<()> {
    Err(Error::Config("auth login not implemented yet".into()))
}
async fn auth_logout() -> Result<()> {
    Err(Error::Config("auth logout not implemented yet".into()))
}
async fn auth_status() -> Result<()> {
    Err(Error::Config("auth status not implemented yet".into()))
}
async fn models() -> Result<()> {
    Err(Error::Config("models listing not implemented yet".into()))
}
async fn config_set_model(_id: &str) -> Result<()> {
    Err(Error::Config("config set-model not implemented yet".into()))
}
async fn config_get_model() -> Result<()> {
    Err(Error::Config("config get-model not implemented yet".into()))
}

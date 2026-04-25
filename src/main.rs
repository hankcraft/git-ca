mod auth;
mod cli;
mod commit_msg;
mod config;
mod copilot;
mod error;
mod git;

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
        None => commit(cli.model, cli.no_verify).await,
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

async fn commit(model_override: Option<String>, no_verify: bool) -> Result<()> {
    let diff = git::diff::staged_diff()?;
    let http = http_client()?;
    let cfg = config::Config::load()?;
    let model = model_override
        .or(cfg.default_model)
        .unwrap_or_else(|| commit_msg::FALLBACK_MODEL.to_string());
    eprintln!("git-ca: drafting message with {model}…");
    let draft = copilot::call_authed(&http, |client| {
        let model = model.clone();
        let diff = diff.clone();
        async move { commit_msg::generate(&client, &model, &diff).await }
    })
    .await?;
    git::commit::commit_with_editor(&draft, no_verify)
}
fn http_client() -> Result<reqwest::Client> {
    use std::time::Duration;
    // Without a request timeout, a hung GitHub or Copilot endpoint hangs the
    // CLI indefinitely — the retry layer can only kick in on errors. 120s
    // covers slow chat completions on long diffs; 10s connect catches DNS
    // and TCP-level stalls fast.
    reqwest::Client::builder()
        .user_agent(concat!("git-ca/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(Into::into)
}

async fn auth_login() -> Result<()> {
    use auth::device_flow::{self, GITHUB_BASE, VSCODE_COPILOT_CLIENT_ID};

    let http = http_client()?;
    let token = device_flow::run(&http, GITHUB_BASE, VSCODE_COPILOT_CLIENT_ID).await?;

    let mut file = auth::AuthFile::load()?;
    file.github_token = Some(token);
    // Invalidate any cached Copilot token from a prior session.
    file.copilot = None;
    file.save()?;
    println!("Logged in. GitHub token stored.");
    Ok(())
}

async fn auth_logout() -> Result<()> {
    auth::AuthFile::clear()?;
    println!("Tokens cleared.");
    Ok(())
}

async fn auth_status() -> Result<()> {
    let file = auth::AuthFile::load()?;
    match file.github_token.as_deref() {
        Some(_) => println!("GitHub: logged in"),
        None => println!("GitHub: not logged in (run `git ca auth login`)"),
    }
    match file.copilot.as_ref() {
        Some(c) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let remaining = c.expires_at - now;
            if remaining > 0 {
                println!("Copilot token: valid for {remaining}s");
            } else {
                println!("Copilot token: expired (will refresh on next use)");
            }
        }
        None => println!("Copilot token: none cached (will fetch on next use)"),
    }
    Ok(())
}
async fn models() -> Result<()> {
    let http = http_client()?;
    let list = copilot::call_authed(
        &http,
        |client| async move { client.list_chat_models().await },
    )
    .await?;
    if list.is_empty() {
        println!("(no chat models available on this account)");
        return Ok(());
    }
    for m in list {
        let name = m.name.as_deref().unwrap_or("");
        let vendor = m.vendor.as_deref().unwrap_or("");
        println!("{:<30}  {:<14}  {}", m.id, vendor, name);
    }
    Ok(())
}
async fn config_set_model(id: &str) -> Result<()> {
    let http = http_client()?;
    let available = copilot::call_authed(
        &http,
        |client| async move { client.list_chat_models().await },
    )
    .await?;
    if !available.iter().any(|m| m.id == id) {
        let ids: Vec<String> = available.into_iter().map(|m| m.id).collect();
        return Err(Error::Config(format!(
            "model `{id}` not available — try one of: {}",
            ids.join(", ")
        )));
    }
    let mut cfg = config::Config::load()?;
    cfg.default_model = Some(id.to_string());
    cfg.save()?;
    println!("Default model set to {id}.");
    Ok(())
}

async fn config_get_model() -> Result<()> {
    let cfg = config::Config::load()?;
    match cfg.default_model.as_deref() {
        Some(m) => println!("{m}"),
        None => println!("(none — defaulting to {})", commit_msg::FALLBACK_MODEL),
    }
    Ok(())
}

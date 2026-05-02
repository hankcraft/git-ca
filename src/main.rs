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
        None => commit(cli.model, cli.no_verify, cli.yes).await,
        Some(Command::Auth { action }) => match action {
            AuthAction::Login { account } => auth_login(account).await,
            AuthAction::SetToken { account, token } => auth_set_token(&account, &token).await,
            AuthAction::Logout { account } => auth_logout(account).await,
            AuthAction::Use { account } => auth_use(&account).await,
            AuthAction::Status => auth_status().await,
        },
        Some(Command::Models) => models().await,
        Some(Command::Config { action }) => match action {
            ConfigAction::List => config_list().await,
            ConfigAction::SetModel { id } => config_set_model(&id).await,
            ConfigAction::GetModel => config_get_model().await,
            ConfigAction::SetAutoAccept { value } => config_set_auto_accept(value).await,
            ConfigAction::GetAutoAccept => config_get_auto_accept().await,
        },
    }
}

async fn commit(model_override: Option<String>, no_verify: bool, yes: bool) -> Result<()> {
    git::ensure_work_tree()?;
    let diff = git::diff::staged_diff()?;
    let http = http_client()?;
    let cfg = config::Config::load()?;
    let auto_accept = yes || cfg.auto_accept;
    let model = model_override
        .or_else(|| cfg.default_model.clone())
        .unwrap_or_else(|| commit_msg::FALLBACK_MODEL.to_string());
    eprintln!("git-ca: drafting message with {model}…");
    let draft = copilot::call_authed(&http, |client| {
        let model = model.clone();
        let diff = diff.clone();
        async move { commit_msg::generate(&client, &model, &diff).await }
    })
    .await?;
    if auto_accept {
        git::commit::commit_generated(&draft, no_verify)
    } else {
        git::commit::commit_with_editor(&draft, no_verify)
    }
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

async fn auth_login(account: Option<String>) -> Result<()> {
    use auth::device_flow::{self, GITHUB_BASE, VSCODE_COPILOT_CLIENT_ID};

    let account = account.unwrap_or_else(|| "default".to_string());
    let http = http_client()?;
    let token = device_flow::run(&http, GITHUB_BASE, VSCODE_COPILOT_CLIENT_ID).await?;

    let mut file = auth::AuthFile::load()?;
    file.set_copilot_github_token(&account, token);
    file.save()?;
    println!("Logged in as {account}. GitHub token stored.");
    Ok(())
}

async fn auth_set_token(account: &str, token: &str) -> Result<()> {
    let token = token.trim();
    if token.is_empty() {
        return Err(Error::Config("token cannot be empty".to_string()));
    }

    let mut file = auth::AuthFile::load()?;
    file.set_copilot_github_token(account, token.to_string());
    file.save()?;
    println!("Token stored for {account}.");
    Ok(())
}

async fn auth_logout(account: Option<String>) -> Result<()> {
    match account {
        Some(account) => {
            let mut file = auth::AuthFile::load()?;
            file.remove_account(&account);
            file.save()?;
            println!("Account {account} cleared.");
        }
        None => {
            auth::AuthFile::clear()?;
            println!("Tokens cleared.");
        }
    }
    Ok(())
}

async fn auth_use(account: &str) -> Result<()> {
    let mut file = auth::AuthFile::load()?;
    file.set_active_account(account.to_string())?;
    file.save()?;
    println!("Active account set to {account}.");
    Ok(())
}

async fn auth_status() -> Result<()> {
    let file = auth::AuthFile::load()?;
    let active = file.active_account();
    match active.and_then(|account| account.github_token()) {
        Some(_) => println!("GitHub: logged in"),
        None => println!("GitHub: not logged in (run `git ca auth login`)"),
    }
    if let Some(account) = active {
        println!("Active account: {}", account.name);
    }
    let accounts: Vec<&str> = file.account_names().collect();
    if !accounts.is_empty() {
        println!("Accounts: {}", accounts.join(", "));
    }
    match active.and_then(|account| account.copilot_cache()) {
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

async fn config_list() -> Result<()> {
    let cfg = config::Config::load()?;
    for line in config_list_lines(&cfg) {
        println!("{line}");
    }
    Ok(())
}

fn config_list_lines(cfg: &config::Config) -> Vec<String> {
    let mut lines = Vec::new();
    match cfg.default_model.as_deref() {
        Some(m) => lines.push(format!("default_model: {m}")),
        None => lines.push(format!(
            "default_model: {} (fallback)",
            commit_msg::FALLBACK_MODEL
        )),
    }
    lines.push(format!("auto_accept: {}", cfg.auto_accept));
    lines
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

async fn config_set_auto_accept(value: bool) -> Result<()> {
    let mut cfg = config::Config::load()?;
    cfg.auto_accept = value;
    cfg.save()?;
    println!("Auto accept set to {value}.");
    Ok(())
}

async fn config_get_auto_accept() -> Result<()> {
    let cfg = config::Config::load()?;
    println!("{}", cfg.auto_accept);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_config_list_with_fallback_model() {
        let cfg = config::Config {
            default_model: None,
            auto_accept: false,
        };

        assert_eq!(
            config_list_lines(&cfg),
            vec![
                format!("default_model: {} (fallback)", commit_msg::FALLBACK_MODEL),
                "auto_accept: false".to_string(),
            ]
        );
    }
}

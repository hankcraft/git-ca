mod auth;
mod cli;
mod codex;
mod commit_msg;
mod config;
mod copilot;
mod error;
mod git;

use clap::Parser;
use cli::{AuthAction, Cli, Command, ConfigAction, Provider};
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
            AuthAction::Login { provider, account } => auth_login(provider, account).await,
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

    let provider = active_provider()?;
    let model = model_override
        .or_else(|| cfg.default_model.clone())
        .unwrap_or_else(|| match provider {
            Provider::Copilot => commit_msg::FALLBACK_MODEL.to_string(),
            Provider::Codex => codex::FALLBACK_MODEL.to_string(),
        });
    eprintln!(
        "git-ca: drafting message with {model} ({})…",
        provider_label(provider)
    );

    let messages = commit_msg::prompt::build(&diff);
    let raw = match provider {
        Provider::Copilot => {
            copilot::call_authed(&http, |client| {
                let model = model.clone();
                let messages = messages.clone();
                async move { client.chat(&model, &messages).await }
            })
            .await?
        }
        Provider::Codex => {
            codex::call_authed(&http, |client| {
                let model = model.clone();
                let messages = messages.clone();
                async move { client.chat(&model, &messages).await }
            })
            .await?
        }
    };
    let draft = commit_msg::strip_code_fences(&raw);
    if auto_accept {
        git::commit::commit_generated(&draft, no_verify)
    } else {
        git::commit::commit_with_editor(&draft, no_verify)
    }
}

/// Resolve the active account's provider from the on-disk auth file. Errors
/// when no account exists so the user is told to log in instead of getting a
/// silent default.
fn active_provider() -> Result<Provider> {
    let file = auth::AuthFile::load()?;
    let active = file.active_account().ok_or(Error::NotAuthenticated)?;
    Ok(match &active.credential {
        auth::store::Credential::Copilot { .. } => Provider::Copilot,
        auth::store::Credential::Codex { .. } => Provider::Codex,
    })
}

fn provider_label(p: Provider) -> &'static str {
    match p {
        Provider::Copilot => "copilot",
        Provider::Codex => "codex",
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

async fn auth_login(provider: Option<Provider>, account: Option<String>) -> Result<()> {
    let provider = match provider {
        Some(p) => p,
        None => resolve_login_provider()?,
    };
    let account = account.unwrap_or_else(|| "default".to_string());
    let http = http_client()?;
    let mut file = auth::AuthFile::load()?;

    match provider {
        Provider::Copilot => {
            use auth::device_flow::{self, GITHUB_BASE, VSCODE_COPILOT_CLIENT_ID};
            let token = device_flow::run(&http, GITHUB_BASE, VSCODE_COPILOT_CLIENT_ID).await?;
            file.set_copilot_github_token(&account, token);
            file.save()?;
            println!("Logged in as {account} (copilot). GitHub token stored.");
        }
        Provider::Codex => {
            let tokens = auth::codex::oauth::run(&http).await?;
            file.set_codex_tokens(&account, tokens);
            file.save()?;
            println!("Logged in as {account} (codex). ChatGPT tokens stored.");
        }
    }
    Ok(())
}

/// Resolve the provider for `auth login` when `--provider` was omitted.
/// Prompts on a TTY so users discover that two backends exist; defaults to
/// Copilot in non-interactive contexts (CI, piped stdin) so scripts that
/// previously relied on the implicit default keep working.
fn resolve_login_provider() -> Result<Provider> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return Ok(Provider::Copilot);
    }
    use std::io::{self, BufRead, Write};
    println!("Choose auth provider:");
    println!("  [1] copilot  GitHub Copilot via device flow (default)");
    println!("  [2] codex    OpenAI Codex via ChatGPT OAuth");
    print!("provider [1]: ");
    io::stdout().flush().ok();
    let mut line = String::new();
    let stdin = io::stdin();
    stdin.lock().read_line(&mut line)?;
    parse_provider_input(&line).ok_or_else(|| {
        Error::Config(format!(
            "unrecognised provider `{}` — pass --provider copilot|codex explicitly",
            line.trim()
        ))
    })
}

fn parse_provider_input(s: &str) -> Option<Provider> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "1" | "copilot" => Some(Provider::Copilot),
        "2" | "codex" => Some(Provider::Codex),
        _ => None,
    }
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
    match active {
        Some(account) => {
            println!(
                "Active account: {} ({})",
                account.name,
                account.provider_label()
            );
        }
        None => {
            println!("Not logged in (run `git ca auth login`)");
        }
    }
    let accounts: Vec<String> = file
        .accounts
        .values()
        .map(|a| format!("{} ({})", a.name, a.provider_label()))
        .collect();
    if !accounts.is_empty() {
        println!("Accounts: {}", accounts.join(", "));
    }
    if let Some(account) = active {
        match &account.credential {
            auth::store::Credential::Copilot { copilot_cache, .. } => match copilot_cache {
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
            },
            auth::store::Credential::Codex {
                tokens,
                last_refresh,
            } => {
                match tokens.account_id.as_deref() {
                    Some(id) => println!("ChatGPT account: {id}"),
                    None => println!("ChatGPT account: (none linked)"),
                }
                match last_refresh {
                    Some(ts) => println!("Last refresh: {ts} (unix)"),
                    None => println!("Last refresh: (never since login)"),
                }
            }
        }
    }
    Ok(())
}
async fn models() -> Result<()> {
    let provider = active_provider()?;
    match provider {
        Provider::Copilot => {
            let http = http_client()?;
            let list =
                copilot::call_authed(
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
        }
        Provider::Codex => {
            // Codex (`/responses` on `chatgpt.com/backend-api/codex`) does not
            // expose a chat-models listing endpoint. Show the slugs that a
            // smoke test confirmed the backend accepts via ChatGPT auth so
            // users can pick one without guessing — note that `gpt-5` and
            // `gpt-5-codex` are explicitly rejected with "not supported" 400s
            // even though they're valid slugs in other contexts.
            println!("(codex backend has no models endpoint — known accepted slugs:)");
            for slug in ["gpt-5.5", "gpt-5.4"] {
                println!("{slug}");
            }
        }
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
    // Only Copilot exposes a model-list endpoint we can validate against.
    // For Codex we accept the id verbatim because the server returns a clear
    // 400 when the slug is unsupported, and we'd otherwise need to maintain
    // a hand-curated allow-list that drifts.
    if let Ok(Provider::Copilot) = active_provider() {
        let http = http_client()?;
        let available =
            copilot::call_authed(
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

    #[test]
    fn parse_provider_input_accepts_blank_and_copilot_aliases() {
        assert!(matches!(parse_provider_input(""), Some(Provider::Copilot)));
        assert!(matches!(
            parse_provider_input("\n"),
            Some(Provider::Copilot)
        ));
        assert!(matches!(parse_provider_input("1"), Some(Provider::Copilot)));
        assert!(matches!(
            parse_provider_input("copilot"),
            Some(Provider::Copilot)
        ));
        assert!(matches!(
            parse_provider_input("  COPILOT  "),
            Some(Provider::Copilot)
        ));
    }

    #[test]
    fn parse_provider_input_accepts_codex_aliases() {
        assert!(matches!(parse_provider_input("2"), Some(Provider::Codex)));
        assert!(matches!(
            parse_provider_input("codex"),
            Some(Provider::Codex)
        ));
        assert!(matches!(
            parse_provider_input("Codex\n"),
            Some(Provider::Codex)
        ));
    }

    #[test]
    fn parse_provider_input_rejects_unknown_value() {
        assert!(parse_provider_input("3").is_none());
        assert!(parse_provider_input("openai").is_none());
        assert!(parse_provider_input("foo").is_none());
    }
}

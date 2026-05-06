mod auth;
mod cli;
mod codex;
mod commit_msg;
mod config;
mod copilot;
mod error;
mod git;
mod pr_msg;

use clap::Parser;
use cli::{AuthAction, Cli, Command, ConfigAction, PrSource, Provider};
use commit_msg::prompt::ChatMessage;
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
        Some(Command::Pr { base, source }) => pull_request(cli.model, cli.yes, base, source).await,
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
            ConfigAction::SetAutoAcceptPr { value } => config_set_auto_accept_pr(value).await,
            ConfigAction::GetAutoAcceptPr => config_get_auto_accept_pr().await,
        },
    }
}

async fn commit(model_override: Option<String>, no_verify: bool, yes: bool) -> Result<()> {
    git::ensure_work_tree()?;
    let diff = git::diff::staged_diff()?;
    let cfg = config::Config::load()?;
    let auto_accept = yes || cfg.auto_accept;
    let system_prompt = load_system_prompt_file(
        &config::paths::commit_system_prompt_file()?,
        "commit system prompt",
    );

    let messages = commit_msg::prompt::build(&diff, system_prompt.as_deref());
    let raw = generate_text(model_override, messages, "drafting message").await?;
    let draft = commit_msg::strip_code_fences(&raw);
    if auto_accept {
        git::commit::commit_generated(&draft, no_verify)
    } else {
        git::commit::commit_with_editor(&draft, no_verify)
    }
}

async fn pull_request(
    model_override: Option<String>,
    yes: bool,
    base: Option<String>,
    source: PrSource,
) -> Result<()> {
    git::ensure_work_tree()?;
    git::pr::ensure_gh_available()?;
    let cfg = config::Config::load()?;
    let auto_accept = pr_auto_accept(yes, &cfg);
    let base = base
        .map(git::pr::BaseBranch::explicit)
        .unwrap_or_else(git::pr::default_base);
    let merge_base = git::pr::merge_base(&base.compare_ref)?;
    let source_text = match source {
        PrSource::Diff => git::pr::branch_diff(&merge_base)?,
        PrSource::Commits => git::pr::commit_log(&merge_base)?,
    };
    let system_prompt =
        load_system_prompt_file(&config::paths::pr_system_prompt_file()?, "PR system prompt");
    let messages = pr_msg::prompt::build(
        source,
        &base.pr_base,
        &source_text,
        system_prompt.as_deref(),
    );
    let raw = generate_text(model_override, messages, "drafting PR message").await?;
    let mut draft = pr_msg::parse_json(&raw)?;
    if !auto_accept {
        draft = git::pr::edit_message(&draft)?;
    }
    git::pr::create_pull_request(&base.pr_base, &draft.title, &draft.body)
}

fn pr_auto_accept(yes: bool, cfg: &config::Config) -> bool {
    yes || cfg.auto_accept_pr
}

fn load_system_prompt_file(path: &std::path::Path, label: &str) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) if content.trim().is_empty() => {
            eprintln!(
                "git-ca: {label} file {} is empty; using built-in prompt",
                path.display()
            );
            None
        }
        Ok(content) => Some(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!(
                "git-ca: unable to read {label} file {}: {e}; using built-in prompt",
                path.display()
            );
            None
        }
    }
}

async fn generate_text(
    model_override: Option<String>,
    messages: Vec<ChatMessage>,
    action: &str,
) -> Result<String> {
    let http = http_client()?;
    let cfg = config::Config::load()?;
    let provider = active_provider()?;
    let model = model_override
        .or_else(|| cfg.default_model.clone())
        .unwrap_or_else(|| match provider {
            Provider::Copilot => commit_msg::FALLBACK_MODEL.to_string(),
            Provider::Codex => codex::FALLBACK_MODEL.to_string(),
        });
    eprintln!(
        "git-ca: {action} with {model} ({})…",
        provider_label(provider)
    );

    Ok(match provider {
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
    })
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
    lines.push(format!("auto_accept_pr: {}", cfg.auto_accept_pr));
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

async fn config_set_auto_accept_pr(value: bool) -> Result<()> {
    let mut cfg = config::Config::load()?;
    cfg.auto_accept_pr = value;
    cfg.save()?;
    println!("PR auto accept set to {value}.");
    Ok(())
}

async fn config_get_auto_accept_pr() -> Result<()> {
    let cfg = config::Config::load()?;
    println!("{}", cfg.auto_accept_pr);
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
            auto_accept_pr: false,
        };

        assert_eq!(
            config_list_lines(&cfg),
            vec![
                format!("default_model: {} (fallback)", commit_msg::FALLBACK_MODEL),
                "auto_accept: false".to_string(),
                "auto_accept_pr: false".to_string(),
            ]
        );
    }

    #[test]
    fn pr_auto_accept_uses_flag_or_pr_config_only() {
        let cfg = config::Config {
            default_model: None,
            auto_accept: true,
            auto_accept_pr: false,
        };

        assert!(!pr_auto_accept(false, &cfg));
        assert!(pr_auto_accept(true, &cfg));

        let cfg = config::Config {
            default_model: None,
            auto_accept: false,
            auto_accept_pr: true,
        };

        assert!(pr_auto_accept(false, &cfg));
    }

    fn tmp_prompt_file(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("git-ca-test-{}-{name}", std::process::id()));
        path
    }

    #[test]
    fn load_system_prompt_file_returns_non_empty_content() {
        let path = tmp_prompt_file("valid-prompt.md");
        std::fs::write(&path, "custom prompt\n").unwrap();

        assert_eq!(
            load_system_prompt_file(&path, "test prompt").as_deref(),
            Some("custom prompt\n")
        );

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn load_system_prompt_file_ignores_missing_file() {
        let path = tmp_prompt_file("missing-prompt.md");
        let _ = std::fs::remove_file(&path);

        assert!(load_system_prompt_file(&path, "test prompt").is_none());
    }

    #[test]
    fn load_system_prompt_file_ignores_empty_file() {
        let path = tmp_prompt_file("empty-prompt.md");
        std::fs::write(&path, " \n\t").unwrap();

        assert!(load_system_prompt_file(&path, "test prompt").is_none());

        std::fs::remove_file(path).unwrap();
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

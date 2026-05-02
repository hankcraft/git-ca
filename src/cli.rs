use clap::{Parser, Subcommand, ValueEnum};

/// Authentication backend selectable on `auth login`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Provider {
    /// GitHub Copilot via device-flow OAuth (default).
    #[default]
    Copilot,
    /// OpenAI Codex via ChatGPT OAuth (PKCE, loopback callback).
    Codex,
}

#[derive(Debug, Parser)]
#[command(
    name = "git-ca",
    version,
    about = "Draft git commit messages with GitHub Copilot",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Skip pre-commit and commit-msg hooks.
    #[arg(short = 'n', long = "no-verify", global = true)]
    pub no_verify: bool,

    /// Copilot model id to use for drafting (overrides the persisted default).
    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,

    /// Commit the generated message without opening the editor.
    #[arg(
        short = 'y',
        long = "yes",
        visible_alias = "auto-accept",
        global = true
    )]
    pub yes: bool,
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
    /// Log in via the selected provider's OAuth flow.
    Login {
        /// Auth backend: `copilot` (GitHub device flow) or `codex` (ChatGPT
        /// OAuth via loopback). Defaults to `copilot`.
        #[arg(long, value_enum, default_value_t = Provider::Copilot)]
        provider: Provider,
        account: Option<String>,
    },
    /// Store a GitHub token manually instead of using device flow.
    SetToken {
        /// Account name to store the token under.
        #[arg(long, default_value = "default")]
        account: String,
        /// GitHub token with Copilot access.
        token: String,
    },
    /// Forget locally stored tokens.
    ///
    /// This only deletes the on-disk credentials. To revoke the GitHub OAuth
    /// grant server-side, visit https://github.com/settings/applications.
    Logout { account: Option<String> },
    /// Switch the account used by commands that call Copilot.
    Use { account: String },
    /// Show auth state and Copilot token TTL.
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print all persisted config values.
    List,
    /// Set the default model used when `--model` is not passed.
    SetModel { id: String },
    /// Print the default model (if any).
    GetModel,
    /// Set whether generated messages are committed without opening the editor.
    SetAutoAccept {
        #[arg(action = clap::ArgAction::Set)]
        value: bool,
    },
    /// Print whether generated messages are committed without opening the editor.
    GetAutoAccept,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_yes_long_flag() {
        let cli = Cli::try_parse_from(["git-ca", "--yes"]).unwrap();

        assert!(cli.yes);
    }

    #[test]
    fn parses_model_short_flag() {
        let cli = Cli::try_parse_from(["git-ca", "-m", "gpt-4o"]).unwrap();

        assert_eq!(cli.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn parses_yes_short_flag() {
        let cli = Cli::try_parse_from(["git-ca", "-y"]).unwrap();

        assert!(cli.yes);
    }

    #[test]
    fn parses_auto_accept_alias() {
        let cli = Cli::try_parse_from(["git-ca", "--auto-accept"]).unwrap();

        assert!(cli.yes);
    }

    #[test]
    fn parses_set_auto_accept_value() {
        let cli = Cli::try_parse_from(["git-ca", "config", "set-auto-accept", "true"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Config {
                action: ConfigAction::SetAutoAccept { value: true }
            })
        ));
    }

    #[test]
    fn parses_config_list() {
        let cli = Cli::try_parse_from(["git-ca", "config", "list"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Config {
                action: ConfigAction::List
            })
        ));
    }

    #[test]
    fn parses_auth_login_account_name() {
        let cli = Cli::try_parse_from(["git-ca", "auth", "login", "work"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Auth {
                action: AuthAction::Login { provider: Provider::Copilot, account }
            }) if account.as_deref() == Some("work")
        ));
    }

    #[test]
    fn parses_auth_login_codex_provider() {
        let cli = Cli::try_parse_from(["git-ca", "auth", "login", "--provider", "codex"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Auth {
                action: AuthAction::Login {
                    provider: Provider::Codex,
                    account: None,
                }
            })
        ));
    }

    #[test]
    fn parses_auth_login_codex_provider_with_account() {
        let cli =
            Cli::try_parse_from(["git-ca", "auth", "login", "--provider", "codex", "personal"])
                .unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Auth {
                action: AuthAction::Login {
                    provider: Provider::Codex,
                    account,
                }
            }) if account.as_deref() == Some("personal")
        ));
    }

    #[test]
    fn parses_auth_set_token_default_account() {
        let cli = Cli::try_parse_from(["git-ca", "auth", "set-token", "gho_manual"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Auth {
                action: AuthAction::SetToken { account, token }
            }) if account == "default" && token == "gho_manual"
        ));
    }

    #[test]
    fn parses_auth_set_token_named_account() {
        let cli = Cli::try_parse_from([
            "git-ca",
            "auth",
            "set-token",
            "--account",
            "work",
            "gho_manual",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Auth {
                action: AuthAction::SetToken { account, token }
            }) if account == "work" && token == "gho_manual"
        ));
    }

    #[test]
    fn parses_auth_use_account_name() {
        let cli = Cli::try_parse_from(["git-ca", "auth", "use", "personal"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Auth {
                action: AuthAction::Use { account }
            }) if account == "personal"
        ));
    }

    #[test]
    fn git_help_man_page_is_packaged() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs")
            .join("man")
            .join("git-ca.1");
        let man_page = std::fs::read_to_string(path).unwrap();

        assert!(man_page.contains(".TH GIT-CA 1"));
        assert!(man_page.contains("git ca \\-h"));
        assert!(man_page.contains("git ca \\--yes"));
    }
}

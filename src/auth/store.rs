use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{self, paths};
use crate::error::{Error, Result};

const DEFAULT_ACCOUNT: &str = "default";

/// On-disk schema for `auth.json`. Kept separate from `Config` so rotating
/// tokens does not rewrite unrelated user preferences.
#[derive(Debug, Default, Clone, Serialize)]
pub struct AuthFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub accounts: BTreeMap<String, AccountAuth>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountAuth {
    pub name: String,
    pub credential: Credential,
}

/// Provider-specific credentials. Tagged on disk with `"provider": "copilot"`
/// or `"provider": "codex"` so the variant is unambiguous regardless of which
/// fields happen to be present.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum Credential {
    Copilot {
        github_token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        copilot_cache: Option<CopilotCache>,
    },
    Codex {
        tokens: ChatGptTokens,
        /// Unix epoch seconds of the most recent successful refresh. Lets us
        /// skip refresh when the token was rotated very recently.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_refresh: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGptTokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotCache {
    pub token: String,
    /// Unix epoch seconds.
    pub expires_at: i64,
}

#[derive(Debug, Default, Deserialize)]
struct RawAccountAuth {
    #[serde(default)]
    name: String,
    #[serde(default)]
    credential: Option<Credential>,
    /// Legacy: GitHub token stored directly on the account record.
    #[serde(default)]
    github_token: Option<String>,
    /// Legacy: Copilot cache stored directly on the account record.
    #[serde(default)]
    copilot: Option<CopilotCache>,
}

impl<'de> Deserialize<'de> for AccountAuth {
    fn deserialize<D>(d: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawAccountAuth::deserialize(d)?;
        let credential = match raw.credential {
            Some(c) => c,
            None => match raw.github_token {
                Some(token) => Credential::Copilot {
                    github_token: token,
                    copilot_cache: raw.copilot,
                },
                None => {
                    return Err(serde::de::Error::custom("account is missing credential"));
                }
            },
        };
        Ok(AccountAuth {
            name: raw.name,
            credential,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawAuthFile {
    #[serde(default)]
    active_account: Option<String>,
    #[serde(default)]
    accounts: BTreeMap<String, AccountAuth>,
    /// Legacy single-account shape: token + cache at the top level, no
    /// `accounts` map.
    #[serde(default)]
    github_token: Option<String>,
    #[serde(default)]
    copilot: Option<CopilotCache>,
}

impl<'de> Deserialize<'de> for AuthFile {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawAuthFile::deserialize(deserializer)?;
        Ok(Self::from(raw))
    }
}

impl From<RawAuthFile> for AuthFile {
    fn from(raw: RawAuthFile) -> Self {
        let mut accounts = raw.accounts;
        for (name, account) in &mut accounts {
            if account.name.is_empty() {
                account.name = name.clone();
            }
        }
        if accounts.is_empty() {
            if let Some(token) = raw.github_token {
                accounts.insert(
                    DEFAULT_ACCOUNT.to_string(),
                    AccountAuth {
                        name: DEFAULT_ACCOUNT.to_string(),
                        credential: Credential::Copilot {
                            github_token: token,
                            copilot_cache: raw.copilot,
                        },
                    },
                );
            }
        }
        let active_account = raw
            .active_account
            .or_else(|| accounts.keys().next().cloned());
        Self {
            active_account,
            accounts,
        }
    }
}

impl AuthFile {
    pub fn load() -> Result<Self> {
        config::read_json_or_default(&paths::auth_file()?)
    }

    pub fn save(&self) -> Result<()> {
        paths::ensure_config_dir()?;
        config::write_json_0600(&paths::auth_file()?, self)
    }

    pub fn clear() -> Result<()> {
        let path = paths::auth_file()?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn active_account(&self) -> Option<&AccountAuth> {
        self.active_account
            .as_deref()
            .and_then(|name| self.accounts.get(name))
    }

    pub fn active_account_mut(&mut self) -> Option<&mut AccountAuth> {
        let name = self.active_account.clone()?;
        self.accounts.get_mut(&name)
    }

    /// Store a GitHub token for a Copilot account, replacing any prior
    /// credential so `set-token` cannot leave a stale Codex/Copilot mix on
    /// disk.
    pub fn set_copilot_github_token(&mut self, name: &str, token: String) {
        self.accounts.insert(
            name.to_string(),
            AccountAuth {
                name: name.to_string(),
                credential: Credential::Copilot {
                    github_token: token,
                    copilot_cache: None,
                },
            },
        );
        self.active_account = Some(name.to_string());
    }

    /// Store ChatGPT OAuth tokens for a Codex account, replacing any prior
    /// credential.
    pub fn set_codex_tokens(&mut self, name: &str, tokens: ChatGptTokens) {
        self.accounts.insert(
            name.to_string(),
            AccountAuth {
                name: name.to_string(),
                credential: Credential::Codex {
                    tokens,
                    last_refresh: None,
                },
            },
        );
        self.active_account = Some(name.to_string());
    }

    pub fn set_active_account(&mut self, name: String) -> Result<()> {
        if !self.accounts.contains_key(&name) {
            return Err(Error::Config(format!("account `{name}` is not logged in")));
        }
        self.active_account = Some(name);
        Ok(())
    }

    pub fn remove_account(&mut self, name: &str) {
        self.accounts.remove(name);
        if self.active_account.as_deref() == Some(name) {
            self.active_account = self.accounts.keys().next().cloned();
        }
    }
}

impl AccountAuth {
    pub fn provider_label(&self) -> &'static str {
        match &self.credential {
            Credential::Copilot { .. } => "copilot",
            Credential::Codex { .. } => "codex",
        }
    }

    pub fn github_token(&self) -> Option<&str> {
        match &self.credential {
            Credential::Copilot { github_token, .. } => Some(github_token.as_str()),
            _ => None,
        }
    }

    pub fn copilot_cache(&self) -> Option<&CopilotCache> {
        match &self.credential {
            Credential::Copilot { copilot_cache, .. } => copilot_cache.as_ref(),
            _ => None,
        }
    }

    pub fn set_copilot_cache(&mut self, cache: CopilotCache) -> Result<()> {
        match &mut self.credential {
            Credential::Copilot { copilot_cache, .. } => {
                *copilot_cache = Some(cache);
                Ok(())
            }
            Credential::Codex { .. } => Err(Error::NotAuthenticated),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_auth_file() {
        let tmp = std::env::temp_dir().join(format!("git-ca-auth-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let file = AuthFile {
            active_account: Some("default".into()),
            accounts: BTreeMap::from([(
                "default".into(),
                AccountAuth {
                    name: "default".into(),
                    credential: Credential::Copilot {
                        github_token: "gho_xxx".into(),
                        copilot_cache: Some(CopilotCache {
                            token: "cop_xxx".into(),
                            expires_at: 1_700_000_000,
                        }),
                    },
                },
            )]),
        };
        config::write_json_0600(&tmp, &file).unwrap();
        let loaded: AuthFile = config::read_json_or_default(&tmp).unwrap();
        assert_eq!(
            loaded.active_account().unwrap().github_token(),
            Some("gho_xxx")
        );
        assert_eq!(
            loaded
                .active_account()
                .unwrap()
                .copilot_cache()
                .unwrap()
                .token,
            "cop_xxx"
        );
        std::fs::remove_file(&tmp).unwrap();
    }

    #[test]
    fn stores_multiple_named_accounts() {
        let mut file = AuthFile::default();

        file.set_copilot_github_token("work", "gho_work".into());
        file.set_copilot_github_token("personal", "gho_personal".into());
        file.set_active_account("personal".into()).unwrap();

        assert_eq!(file.active_account().unwrap().name, "personal");
        assert_eq!(
            file.active_account().unwrap().github_token(),
            Some("gho_personal")
        );
        assert_eq!(
            file.accounts.get("work").unwrap().github_token(),
            Some("gho_work")
        );
    }

    #[test]
    fn legacy_single_account_auth_file_loads_as_default_account() {
        let json = r#"{"github_token":"gho_legacy","copilot":{"token":"cop_legacy","expires_at":1700000000}}"#;

        let loaded: AuthFile = serde_json::from_str(json).unwrap();

        assert_eq!(loaded.active_account().unwrap().name, "default");
        assert_eq!(
            loaded.active_account().unwrap().github_token(),
            Some("gho_legacy")
        );
        assert_eq!(
            loaded
                .active_account()
                .unwrap()
                .copilot_cache()
                .unwrap()
                .token,
            "cop_legacy"
        );
    }

    #[test]
    fn legacy_account_record_without_provider_tag_loads_as_copilot() {
        let json = r#"{
            "active_account": "default",
            "accounts": {
                "default": {
                    "name": "default",
                    "github_token": "gho_legacy",
                    "copilot": { "token": "cop_legacy", "expires_at": 1700000000 }
                }
            }
        }"#;

        let loaded: AuthFile = serde_json::from_str(json).unwrap();

        let active = loaded.active_account().unwrap();
        assert!(matches!(active.credential, Credential::Copilot { .. }));
        assert_eq!(active.github_token(), Some("gho_legacy"));
    }

    #[test]
    fn codex_account_round_trips_with_provider_tag() {
        let codex_tokens = ChatGptTokens {
            access_token: "at_xxx".into(),
            refresh_token: "rt_xxx".into(),
            id_token: Some("id_xxx".into()),
            account_id: Some("acct_123".into()),
        };
        let mut accounts = BTreeMap::new();
        accounts.insert(
            "personal".into(),
            AccountAuth {
                name: "personal".into(),
                credential: Credential::Codex {
                    tokens: codex_tokens,
                    last_refresh: None,
                },
            },
        );
        let file = AuthFile {
            active_account: Some("personal".into()),
            accounts,
        };

        let serialized = serde_json::to_string(&file).unwrap();
        assert!(
            serialized.contains(r#""provider":"codex""#),
            "expected provider tag, got {serialized}"
        );

        let loaded: AuthFile = serde_json::from_str(&serialized).unwrap();
        let active = loaded.active_account().unwrap();
        assert_eq!(active.github_token(), None);
        match &active.credential {
            Credential::Codex {
                tokens,
                last_refresh,
            } => {
                assert_eq!(tokens.access_token, "at_xxx");
                assert_eq!(tokens.refresh_token, "rt_xxx");
                assert_eq!(tokens.account_id.as_deref(), Some("acct_123"));
                assert!(last_refresh.is_none());
            }
            other => panic!("expected Codex credential, got {other:?}"),
        }
    }

    #[test]
    fn mixed_provider_accounts_coexist() {
        let mut file = AuthFile::default();
        file.set_copilot_github_token("work", "gho_work".into());
        file.accounts.insert(
            "personal".into(),
            AccountAuth {
                name: "personal".into(),
                credential: Credential::Codex {
                    tokens: ChatGptTokens {
                        access_token: "at_xxx".into(),
                        refresh_token: "rt_xxx".into(),
                        id_token: None,
                        account_id: None,
                    },
                    last_refresh: None,
                },
            },
        );

        let serialized = serde_json::to_string(&file).unwrap();
        let loaded: AuthFile = serde_json::from_str(&serialized).unwrap();

        assert!(matches!(
            loaded.accounts.get("work").unwrap().credential,
            Credential::Copilot { .. }
        ));
        assert!(matches!(
            loaded.accounts.get("personal").unwrap().credential,
            Credential::Codex { .. }
        ));
    }

    #[test]
    fn set_copilot_cache_rejects_codex_account() {
        let mut account = AccountAuth {
            name: "personal".into(),
            credential: Credential::Codex {
                tokens: ChatGptTokens {
                    access_token: "at".into(),
                    refresh_token: "rt".into(),
                    id_token: None,
                    account_id: None,
                },
                last_refresh: None,
            },
        };

        let err = account
            .set_copilot_cache(CopilotCache {
                token: "x".into(),
                expires_at: 0,
            })
            .unwrap_err();
        assert!(matches!(err, Error::NotAuthenticated));
    }
}

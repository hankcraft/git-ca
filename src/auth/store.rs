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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AccountAuth {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copilot: Option<CopilotCache>,
}

#[derive(Debug, Default, Deserialize)]
struct RawAuthFile {
    #[serde(default)]
    active_account: Option<String>,
    #[serde(default)]
    accounts: BTreeMap<String, AccountAuth>,
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
        if accounts.is_empty() && (raw.github_token.is_some() || raw.copilot.is_some()) {
            accounts.insert(
                DEFAULT_ACCOUNT.to_string(),
                AccountAuth {
                    name: DEFAULT_ACCOUNT.to_string(),
                    github_token: raw.github_token,
                    copilot: raw.copilot,
                },
            );
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotCache {
    pub token: String,
    /// Unix epoch seconds.
    pub expires_at: i64,
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

    pub fn account_names(&self) -> impl Iterator<Item = &str> {
        self.accounts.keys().map(String::as_str)
    }

    pub fn set_github_token(&mut self, name: &str, token: String) {
        let account = self
            .accounts
            .entry(name.to_string())
            .or_insert_with(|| AccountAuth {
                name: name.to_string(),
                github_token: None,
                copilot: None,
            });
        account.github_token = Some(token);
        account.copilot = None;
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
                    github_token: Some("gho_xxx".into()),
                    copilot: Some(CopilotCache {
                        token: "cop_xxx".into(),
                        expires_at: 1_700_000_000,
                    }),
                },
            )]),
        };
        config::write_json_0600(&tmp, &file).unwrap();
        let loaded: AuthFile = config::read_json_or_default(&tmp).unwrap();
        assert_eq!(
            loaded.active_account().unwrap().github_token.as_deref(),
            Some("gho_xxx")
        );
        assert_eq!(
            loaded
                .active_account()
                .unwrap()
                .copilot
                .as_ref()
                .unwrap()
                .token,
            "cop_xxx"
        );
        std::fs::remove_file(&tmp).unwrap();
    }

    #[test]
    fn stores_multiple_named_accounts() {
        let mut file = AuthFile::default();

        file.set_github_token("work", "gho_work".into());
        file.set_github_token("personal", "gho_personal".into());
        file.set_active_account("personal".into()).unwrap();

        assert_eq!(file.active_account().unwrap().name, "personal");
        assert_eq!(
            file.active_account().unwrap().github_token.as_deref(),
            Some("gho_personal")
        );
        assert_eq!(
            file.accounts.get("work").unwrap().github_token.as_deref(),
            Some("gho_work")
        );
    }

    #[test]
    fn legacy_single_account_auth_file_loads_as_default_account() {
        let json = r#"{"github_token":"gho_legacy","copilot":{"token":"cop_legacy","expires_at":1700000000}}"#;

        let loaded: AuthFile = serde_json::from_str(json).unwrap();

        assert_eq!(loaded.active_account().unwrap().name, "default");
        assert_eq!(
            loaded.active_account().unwrap().github_token.as_deref(),
            Some("gho_legacy")
        );
        assert_eq!(
            loaded
                .active_account()
                .unwrap()
                .copilot
                .as_ref()
                .unwrap()
                .token,
            "cop_legacy"
        );
    }
}

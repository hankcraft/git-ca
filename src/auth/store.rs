use serde::{Deserialize, Serialize};

use crate::config::{self, paths};
use crate::error::Result;

/// On-disk schema for `auth.json`. Kept separate from `Config` so rotating
/// tokens does not rewrite unrelated user preferences.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AuthFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copilot: Option<CopilotCache>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_auth_file() {
        let tmp = std::env::temp_dir().join(format!("git-ca-auth-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let file = AuthFile {
            github_token: Some("gho_xxx".into()),
            copilot: Some(CopilotCache {
                token: "cop_xxx".into(),
                expires_at: 1_700_000_000,
            }),
        };
        config::write_json_0600(&tmp, &file).unwrap();
        let loaded: AuthFile = config::read_json_or_default(&tmp).unwrap();
        assert_eq!(loaded.github_token.as_deref(), Some("gho_xxx"));
        assert_eq!(loaded.copilot.as_ref().unwrap().token, "cop_xxx");
        std::fs::remove_file(&tmp).unwrap();
    }
}

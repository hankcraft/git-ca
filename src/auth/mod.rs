pub mod codex;
pub mod copilot_token;
pub mod device_flow;
pub mod store;

pub use store::AuthFile;

use crate::error::Result;

/// Load the auth file and return a valid Copilot token, exchanging/refreshing
/// transparently. Returns the file so callers can persist further changes
/// (e.g. after a forced refresh on 401).
pub async fn ensure_copilot_token(http: &reqwest::Client) -> Result<(String, AuthFile)> {
    let mut file = AuthFile::load()?;
    let token = copilot_token::ensure(http, copilot_token::GITHUB_API_BASE, &mut file).await?;
    Ok((token, file))
}

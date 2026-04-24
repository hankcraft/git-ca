pub mod chat;
pub mod client;
pub mod models;

pub use chat::ChatMessage;
pub use client::Client;
pub use models::Model;

use crate::auth::{self, copilot_token};
use crate::error::{Error, Result};

/// Run an authenticated Copilot call. On HTTP 401 we force a token refresh
/// (the cached short-lived token may have been revoked or rotated) and retry
/// exactly once; further 401s surface as Error::CopilotAuth.
pub async fn call_authed<T, F, Fut>(http: &reqwest::Client, op: F) -> Result<T>
where
    F: Fn(Client) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let (token, mut file) = auth::ensure_copilot_token(http).await?;
    let client = Client::new(http.clone(), &token);
    match op(client).await {
        Ok(v) => Ok(v),
        Err(Error::CopilotAuth) => {
            let token = copilot_token::refresh(http, copilot_token::GITHUB_API_BASE, &mut file)
                .await?;
            let client = Client::new(http.clone(), &token);
            op(client).await
        }
        Err(e) => Err(e),
    }
}

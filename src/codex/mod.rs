pub mod chat;
pub mod client;
pub mod sse;

pub use chat::FALLBACK_MODEL;
pub use client::Client;

use std::time::Duration;

use crate::auth::store::Credential;
use crate::auth::{self, codex::token, AuthFile};
use crate::error::{Error, Result};

/// Run an authenticated Codex call with the same transient-error and reauth
/// pattern used for Copilot. On HTTP 401 we refresh the ChatGPT access token
/// once and retry; on network errors or 5xx we retry twice with 1s/3s backoff.
pub async fn call_authed<T, F, Fut>(http: &reqwest::Client, op: F) -> Result<T>
where
    F: Fn(Client) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let (mut access_token, account_id, mut file) = load_codex_creds()?;
    let mut client = Client::new(http.clone(), &access_token, account_id.as_deref());
    match retry_transient(|| op(client.clone())).await {
        Err(Error::CodexAuth) => {
            access_token = token::refresh(http, &mut file).await?;
            let (_, account_id, _) = load_codex_creds()?;
            client = Client::new(http.clone(), &access_token, account_id.as_deref());
            retry_transient(|| op(client.clone())).await
        }
        other => other,
    }
}

fn load_codex_creds() -> Result<(String, Option<String>, AuthFile)> {
    let file = auth::AuthFile::load()?;
    let (access_token, account_id) = file
        .active_account()
        .and_then(|a| match &a.credential {
            Credential::Codex { tokens, .. } => {
                Some((tokens.access_token.clone(), tokens.account_id.clone()))
            }
            _ => None,
        })
        .ok_or(Error::NotAuthenticated)?;
    Ok((access_token, account_id, file))
}

async fn retry_transient<T, F, Fut>(op: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    const BACKOFF_SECS: [u64; 2] = [1, 3];
    let mut attempt = 0usize;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if attempt < BACKOFF_SECS.len() && is_transient(&e) => {
                eprintln!(
                    "git-ca: transient Codex error ({e}); retrying in {}s",
                    BACKOFF_SECS[attempt]
                );
                tokio::time::sleep(Duration::from_secs(BACKOFF_SECS[attempt])).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_transient(e: &Error) -> bool {
    match e {
        Error::Network(_) => true,
        Error::CodexServer { status, .. } => *status >= 500,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn retry_transient_gives_up_after_two_retries() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let result: Result<()> = retry_transient(|| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(Error::CodexServer {
                    status: 503,
                    body: "x".into(),
                })
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_transient_stops_on_non_transient() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let result: Result<()> = retry_transient(|| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(Error::CodexAuth)
            }
        })
        .await;
        assert!(matches!(result, Err(Error::CodexAuth)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

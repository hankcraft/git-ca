pub mod chat;
pub mod client;
pub mod models;

pub use chat::ChatMessage;
pub use client::Client;

use std::time::Duration;

use crate::auth::{self, copilot_token};
use crate::error::{Error, Result};

/// Run an authenticated Copilot call with transient-error retry. On HTTP 401
/// we force a token refresh and retry once; on network errors or 5xx we retry
/// twice with 1s/3s backoff before surfacing.
pub async fn call_authed<T, F, Fut>(http: &reqwest::Client, op: F) -> Result<T>
where
    F: Fn(Client) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let (token, mut file) = auth::ensure_copilot_token(http).await?;
    let client = Client::new(http.clone(), &token);
    match retry_transient(|| op(client.clone())).await {
        Err(Error::CopilotAuth) => {
            let token =
                copilot_token::refresh(http, copilot_token::GITHUB_API_BASE, &mut file).await?;
            let client = Client::new(http.clone(), &token);
            retry_transient(|| op(client.clone())).await
        }
        other => other,
    }
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
                    "git-ca: transient Copilot error ({e}); retrying in {}s",
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
        Error::CopilotServer { status, .. } => *status >= 500,
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
                Err(Error::CopilotServer {
                    status: 503,
                    body: "oops".into(),
                })
            }
        })
        .await;
        assert!(result.is_err());
        // 1 initial + 2 retries = 3 calls
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
                Err(Error::CopilotAuth)
            }
        })
        .await;
        assert!(matches!(result, Err(Error::CopilotAuth)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_transient_succeeds_after_first_failure() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let result: Result<&str> = retry_transient(|| {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err(Error::CopilotServer {
                        status: 500,
                        body: "x".into(),
                    })
                } else {
                    Ok("ok")
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}

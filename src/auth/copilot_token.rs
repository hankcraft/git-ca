use serde::Deserialize;

use crate::auth::store::{AuthFile, CopilotCache};
use crate::error::{Error, Result};

pub const GITHUB_API_BASE: &str = "https://api.github.com";
const REFRESH_SKEW_SECS: i64 = 60;

#[derive(Debug, Deserialize)]
struct ExchangeResp {
    token: String,
    expires_at: i64,
}

/// Return a valid Copilot API token, exchanging from the stored GitHub token
/// when the cache is missing or within `REFRESH_SKEW_SECS` of expiry.
pub async fn ensure(http: &reqwest::Client, api_base: &str, file: &mut AuthFile) -> Result<String> {
    if let Some(cache) = file
        .active_account()
        .and_then(|account| account.copilot.as_ref())
    {
        if cache.expires_at - now_unix() > REFRESH_SKEW_SECS {
            return Ok(cache.token.clone());
        }
    }
    refresh(http, api_base, file).await
}

/// Force a token exchange, ignoring any cached value. Used on HTTP 401 from
/// Copilot to recover from a revoked token without bouncing the user through
/// device flow again.
pub async fn refresh(
    http: &reqwest::Client,
    api_base: &str,
    file: &mut AuthFile,
) -> Result<String> {
    let gh_token = file
        .active_account()
        .and_then(|account| account.github_token.as_deref())
        .ok_or(Error::NotAuthenticated)?
        .to_string();
    let cache = exchange_token(http, api_base, &gh_token).await?;
    let token = cache.token.clone();
    let account = file.active_account_mut().ok_or(Error::NotAuthenticated)?;
    account.copilot = Some(cache);
    file.save()?;
    Ok(token)
}

/// Pure HTTP exchange: GitHub token in, fresh `CopilotCache` out. Split out so
/// tests can drive it against a mock server without writing to the user's
/// real config directory via `AuthFile::save`.
async fn exchange_token(
    http: &reqwest::Client,
    api_base: &str,
    gh_token: &str,
) -> Result<CopilotCache> {
    let resp = http
        .get(format!("{api_base}/copilot_internal/v2/token"))
        .header("Authorization", format!("token {gh_token}"))
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::CopilotAuth);
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::CopilotServer {
            status: status.as_u16(),
            body,
        });
    }
    let exchange: ExchangeResp = resp.json().await?;
    Ok(CopilotCache {
        token: exchange.token,
        expires_at: exchange.expires_at,
    })
}

fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn ensure_uses_cache_when_valid() {
        let server = MockServer::start().await;
        let mut file = AuthFile {
            active_account: Some("default".into()),
            accounts: std::collections::BTreeMap::from([(
                "default".into(),
                crate::auth::store::AccountAuth {
                    name: "default".into(),
                    github_token: Some("gho_x".into()),
                    copilot: Some(CopilotCache {
                        token: "cached".into(),
                        expires_at: now_unix() + 3600,
                    }),
                },
            )]),
        };
        let http = reqwest::Client::new();
        let token = ensure(&http, &server.uri(), &mut file).await.unwrap();
        assert_eq!(token, "cached");
        // mock server saw no requests
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn exchange_token_returns_cache_on_success() {
        let server = MockServer::start().await;
        let expires = now_unix() + 1800;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .and(header("Authorization", "token gho_x"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "cop_new",
                "expires_at": expires,
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let cache = exchange_token(&http, &server.uri(), "gho_x").await.unwrap();
        assert_eq!(cache.token, "cop_new");
        assert_eq!(cache.expires_at, expires);
    }

    #[tokio::test]
    async fn exchange_token_unauthorized_maps_to_copilot_auth() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let err = exchange_token(&http, &server.uri(), "gho_x")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::CopilotAuth), "got {err:?}");
    }

    #[tokio::test]
    async fn exchange_token_5xx_maps_to_copilot_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let err = exchange_token(&http, &server.uri(), "gho_x")
            .await
            .unwrap_err();
        match err {
            Error::CopilotServer { status, body } => {
                assert_eq!(status, 503);
                assert!(body.contains("upstream down"));
            }
            other => panic!("expected CopilotServer, got {other:?}"),
        }
    }
}

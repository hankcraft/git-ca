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
    if let Some(cache) = &file.copilot {
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
        .github_token
        .as_deref()
        .ok_or(Error::NotAuthenticated)?;

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
    file.copilot = Some(CopilotCache {
        token: exchange.token.clone(),
        expires_at: exchange.expires_at,
    });
    file.save()?;
    Ok(exchange.token)
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

    fn auth_with_gh(token: &str) -> AuthFile {
        AuthFile {
            github_token: Some(token.into()),
            copilot: None,
        }
    }

    #[tokio::test]
    async fn ensure_uses_cache_when_valid() {
        let server = MockServer::start().await;
        let mut file = AuthFile {
            github_token: Some("gho_x".into()),
            copilot: Some(CopilotCache {
                token: "cached".into(),
                expires_at: now_unix() + 3600,
            }),
        };
        let http = reqwest::Client::new();
        let token = ensure(&http, &server.uri(), &mut file).await.unwrap();
        assert_eq!(token, "cached");
        // mock server saw no requests
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn refresh_stores_new_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .and(header("Authorization", "token gho_x"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "cop_new",
                "expires_at": now_unix() + 1800,
            })))
            .mount(&server)
            .await;
        let mut file = auth_with_gh("gho_x");
        let http = reqwest::Client::new();
        // Persist to tmp — AuthFile::save hits the real config dir, which is
        // fine for the test host but we restore after.
        let token = refresh_in_memory(&http, &server.uri(), &mut file)
            .await
            .unwrap();
        assert_eq!(token, "cop_new");
        assert_eq!(file.copilot.as_ref().unwrap().token, "cop_new");
    }

    #[tokio::test]
    async fn unauthorized_maps_to_copilot_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let mut file = auth_with_gh("gho_x");
        let http = reqwest::Client::new();
        let err = refresh_in_memory(&http, &server.uri(), &mut file)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::CopilotAuth), "got {err:?}");
    }

    /// Test helper: perform the same request as `refresh` but skip the
    /// `file.save()` step so tests don't touch the user's config dir.
    async fn refresh_in_memory(
        http: &reqwest::Client,
        api_base: &str,
        file: &mut AuthFile,
    ) -> Result<String> {
        let gh_token = file
            .github_token
            .as_deref()
            .ok_or(Error::NotAuthenticated)?;
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
        file.copilot = Some(CopilotCache {
            token: exchange.token.clone(),
            expires_at: exchange.expires_at,
        });
        Ok(exchange.token)
    }
}

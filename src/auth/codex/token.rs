use serde::{Deserialize, Serialize};

use crate::auth::codex::oauth::{CLIENT_ID, ISSUER_BASE};
use crate::auth::store::{AuthFile, ChatGptTokens, Credential};
use crate::error::{Error, Result};

const TOKEN_PATH: &str = "/oauth/token";

#[derive(Debug, Serialize)]
struct RefreshRequest<'a> {
    client_id: &'a str,
    grant_type: &'a str,
    refresh_token: &'a str,
}

#[derive(Debug, Deserialize)]
struct RefreshResp {
    access_token: String,
    /// OpenAI rotates the refresh token on each refresh; persist the new one
    /// or future refreshes will fail.
    refresh_token: String,
    /// Optional — present when the OIDC scope is set, which it is for us.
    id_token: Option<String>,
}

/// Force a refresh of the active Codex account's access token. Persists the
/// rotated tokens so subsequent calls keep working.
pub async fn refresh(http: &reqwest::Client, file: &mut AuthFile) -> Result<String> {
    refresh_with_base(http, ISSUER_BASE, file).await
}

pub(crate) async fn refresh_with_base(
    http: &reqwest::Client,
    issuer_base: &str,
    file: &mut AuthFile,
) -> Result<String> {
    let refresh_token = file
        .active_account()
        .and_then(|a| match &a.credential {
            Credential::Codex { tokens, .. } => Some(tokens.refresh_token.clone()),
            _ => None,
        })
        .ok_or(Error::NotAuthenticated)?;

    let resp = http
        .post(format!("{issuer_base}{TOKEN_PATH}"))
        .header("Content-Type", "application/json")
        .json(&RefreshRequest {
            client_id: CLIENT_ID,
            grant_type: "refresh_token",
            refresh_token: &refresh_token,
        })
        .send()
        .await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::CodexAuth);
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::CodexServer {
            status: status.as_u16(),
            body,
        });
    }
    let parsed: RefreshResp = resp.json().await?;

    let account_id = match parsed.id_token.as_deref() {
        Some(t) => crate::auth::codex::jwt::chatgpt_account_id(t)?,
        None => file.active_account().and_then(|a| match &a.credential {
            Credential::Codex { tokens, .. } => tokens.account_id.clone(),
            _ => None,
        }),
    };

    let new_tokens = ChatGptTokens {
        access_token: parsed.access_token.clone(),
        refresh_token: parsed.refresh_token,
        id_token: parsed.id_token,
        account_id,
    };

    let account = file.active_account_mut().ok_or(Error::NotAuthenticated)?;
    match &mut account.credential {
        Credential::Codex {
            tokens,
            last_refresh,
        } => {
            *tokens = new_tokens;
            *last_refresh = Some(now_unix());
        }
        Credential::Copilot { .. } => return Err(Error::NotAuthenticated),
    }
    file.save()?;
    Ok(parsed.access_token)
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
    use crate::auth::store::AccountAuth;
    use std::collections::BTreeMap;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn codex_file(refresh_token: &str) -> AuthFile {
        let mut accounts = BTreeMap::new();
        accounts.insert(
            "default".into(),
            AccountAuth {
                name: "default".into(),
                credential: Credential::Codex {
                    tokens: ChatGptTokens {
                        access_token: "old_at".into(),
                        refresh_token: refresh_token.into(),
                        id_token: None,
                        account_id: Some("acct_existing".into()),
                    },
                    last_refresh: None,
                },
            },
        );
        AuthFile {
            active_account: Some("default".into()),
            accounts,
        }
    }

    #[tokio::test]
    async fn refresh_rotates_tokens_and_returns_new_access() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(TOKEN_PATH))
            .and(body_json(serde_json::json!({
                "client_id": CLIENT_ID,
                "grant_type": "refresh_token",
                "refresh_token": "rt_old",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "at_new",
                "refresh_token": "rt_new",
            })))
            .mount(&server)
            .await;

        let mut file = codex_file("rt_old");
        let http = reqwest::Client::new();
        let token = refresh_with_base(&http, &server.uri(), &mut file)
            .await
            .unwrap();
        assert_eq!(token, "at_new");

        match &file.active_account().unwrap().credential {
            Credential::Codex {
                tokens,
                last_refresh,
            } => {
                assert_eq!(tokens.access_token, "at_new");
                assert_eq!(tokens.refresh_token, "rt_new");
                assert_eq!(tokens.account_id.as_deref(), Some("acct_existing"));
                assert!(last_refresh.is_some());
            }
            _ => panic!("expected Codex credential"),
        }
    }

    #[tokio::test]
    async fn refresh_unauthorized_maps_to_codex_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(TOKEN_PATH))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let mut file = codex_file("rt_x");
        let http = reqwest::Client::new();
        let err = refresh_with_base(&http, &server.uri(), &mut file)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::CodexAuth), "got {err:?}");
    }

    #[tokio::test]
    async fn refresh_server_error_maps_to_codex_server() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(TOKEN_PATH))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream"))
            .mount(&server)
            .await;

        let mut file = codex_file("rt_x");
        let http = reqwest::Client::new();
        let err = refresh_with_base(&http, &server.uri(), &mut file)
            .await
            .unwrap_err();
        match err {
            Error::CodexServer { status, body } => {
                assert_eq!(status, 503);
                assert!(body.contains("upstream"));
            }
            other => panic!("expected CodexServer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn refresh_rejects_non_codex_account() {
        let server = MockServer::start().await;
        let mut file = AuthFile {
            active_account: Some("default".into()),
            accounts: BTreeMap::from([(
                "default".into(),
                AccountAuth {
                    name: "default".into(),
                    credential: Credential::Copilot {
                        github_token: "gho".into(),
                        copilot_cache: None,
                    },
                },
            )]),
        };
        let http = reqwest::Client::new();
        let err = refresh_with_base(&http, &server.uri(), &mut file)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::NotAuthenticated));
    }
}

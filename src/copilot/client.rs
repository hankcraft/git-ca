use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};

use crate::error::Error;

pub const API_BASE: &str = "https://api.githubcopilot.com";

/// Headers Copilot's API expects. These imitate the VS Code Copilot Chat
/// extension — a third-party editor id is accepted in practice but will break
/// if GitHub tightens verification.
fn default_headers(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("token ASCII"),
    );
    h.insert("Editor-Version", HeaderValue::from_static("vscode/1.89.0"));
    h.insert(
        "Editor-Plugin-Version",
        HeaderValue::from_static("copilot-chat/0.14.0"),
    );
    h.insert(
        "Copilot-Integration-Id",
        HeaderValue::from_static("vscode-chat"),
    );
    h.insert(
        USER_AGENT,
        HeaderValue::from_static("GitHubCopilotChat/0.14.0"),
    );
    h
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    headers: HeaderMap,
}

impl Client {
    pub fn new(http: reqwest::Client, token: &str) -> Self {
        Self::with_base(http, token, API_BASE.into())
    }

    pub fn with_base(http: reqwest::Client, token: &str, base_url: String) -> Self {
        Self {
            http,
            base_url,
            headers: default_headers(token),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub(crate) fn headers(&self) -> HeaderMap {
        self.headers.clone()
    }
}

/// Map a non-2xx Copilot response to the appropriate typed error. The body is
/// read here so callers don't have to duplicate the 401/429/5xx bookkeeping.
pub(crate) async fn map_error(resp: reqwest::Response) -> Error {
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Error::CopilotAuth;
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        return Error::CopilotRateLimited { retry_after };
    }
    let body = resp.text().await.unwrap_or_default();
    Error::CopilotServer {
        status: status.as_u16(),
        body: body.chars().take(500).collect(),
    }
}

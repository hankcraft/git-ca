use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};

use crate::error::Error;

pub const API_BASE: &str = "https://chatgpt.com/backend-api/codex";

/// Identifier codex itself stamps on its requests. Same caveat as the OAuth
/// flow: the `/responses` endpoint is undocumented and tightening verification
/// is the most likely break vector, so we mimic codex's own value.
const ORIGINATOR: &str = "codex_cli_rs";

/// Build the headers codex sends on every chat request.
fn default_headers(access_token: &str, account_id: Option<&str>, session_id: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {access_token}")).expect("token ASCII"),
    );
    if let Some(account_id) = account_id {
        if let Ok(value) = HeaderValue::from_str(account_id) {
            h.insert("ChatGPT-Account-ID", value);
        }
    }
    h.insert("originator", HeaderValue::from_static(ORIGINATOR));
    if let Ok(value) = HeaderValue::from_str(session_id) {
        h.insert("session_id", value);
    }
    h.insert(
        USER_AGENT,
        HeaderValue::from_str(concat!("git-ca/", env!("CARGO_PKG_VERSION")))
            .expect("user-agent ASCII"),
    );
    h
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    headers: HeaderMap,
    session_id: String,
}

impl Client {
    pub fn new(http: reqwest::Client, access_token: &str, account_id: Option<&str>) -> Self {
        Self::with_base(http, access_token, account_id, API_BASE.into())
    }

    pub fn with_base(
        http: reqwest::Client,
        access_token: &str,
        account_id: Option<&str>,
        base_url: String,
    ) -> Self {
        let session_id = random_uuid_v4();
        Self {
            http,
            base_url,
            headers: default_headers(access_token, account_id, &session_id),
            session_id,
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

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

pub(crate) async fn map_error(resp: reqwest::Response) -> Error {
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Error::CodexAuth;
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        return Error::CodexRateLimited { retry_after };
    }
    let body = resp.text().await.unwrap_or_default();
    Error::CodexServer {
        status: status.as_u16(),
        body: body.chars().take(500).collect(),
    }
}

fn random_uuid_v4() -> String {
    let mut b = [0u8; 16];
    if getrandom::getrandom(&mut b).is_err() {
        return "00000000-0000-4000-8000-000000000000".into();
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_v4_has_correct_version_and_variant_bits() {
        for _ in 0..32 {
            let id = random_uuid_v4();
            // 8-4-4-4-12 hex layout.
            assert_eq!(id.len(), 36);
            assert_eq!(id.as_bytes()[14], b'4', "version nibble {id}");
            let variant = id.as_bytes()[19] as char;
            assert!("89ab".contains(variant), "variant nibble {id}");
        }
    }

    #[test]
    fn headers_include_originator_and_account_id() {
        let h = default_headers("at_xxx", Some("acct_abc"), "sess_1");
        assert_eq!(h.get("authorization").unwrap(), "Bearer at_xxx");
        assert_eq!(h.get("chatgpt-account-id").unwrap(), "acct_abc");
        assert_eq!(h.get("originator").unwrap(), ORIGINATOR);
        assert_eq!(h.get("session_id").unwrap(), "sess_1");
    }

    #[test]
    fn headers_omit_account_id_when_none() {
        let h = default_headers("at_xxx", None, "sess_1");
        assert!(h.get("chatgpt-account-id").is_none());
    }
}

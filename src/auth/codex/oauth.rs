use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::auth::codex::{jwt, pkce};
use crate::auth::store::ChatGptTokens;
use crate::error::{Error, Result};

/// Public OAuth client id codex itself uses. The PKCE flow does not require
/// a client secret.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// OpenAI auth issuer. Override in tests to point at a local mock.
pub const ISSUER_BASE: &str = "https://auth.openai.com";

const AUTHORIZE_PATH: &str = "/oauth/authorize";
const TOKEN_PATH: &str = "/oauth/token";

/// Identifier the codex CLI puts on its OAuth requests. We send the same
/// value because (a) the endpoint is undocumented and tightening verification
/// would be the most likely break vector and (b) we are functionally a codex
/// client. README calls this out.
const ORIGINATOR: &str = "codex_cli_rs";

const SCOPES: &str =
    "openid profile email offline_access api.connectors.read api.connectors.invoke";
const CALLBACK_PATH: &str = "/auth/callback";

/// Ports codex picks from when binding the loopback callback. We try the
/// primary first; if it is busy we fall back so two simultaneous logins do
/// not collide.
const DEFAULT_PORT: u16 = 1455;
const FALLBACK_PORT: u16 = 1457;

/// 15 minutes — same window codex itself uses, long enough for users who
/// have to log in on a different device.
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Run the full PKCE flow against the real OpenAI issuer.
pub async fn run(http: &reqwest::Client) -> Result<ChatGptTokens> {
    run_with_base(http, ISSUER_BASE).await
}

/// Same as [`run`] but takes the issuer base so tests can point at wiremock.
pub async fn run_with_base(http: &reqwest::Client, issuer_base: &str) -> Result<ChatGptTokens> {
    let listener = bind_loopback().await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://localhost:{port}{CALLBACK_PATH}");

    let pkce = pkce::generate()?;
    let state = pkce::random_token(32)?;
    let auth_url = build_authorize_url(issuer_base, &redirect_uri, &pkce.code_challenge, &state)?;

    eprintln!("Open {auth_url} in your browser to log in to ChatGPT.");
    open_url_best_effort(auth_url.as_str());

    let code = await_callback(listener, &state).await?;
    let tokens =
        exchange_code(http, issuer_base, &code, &redirect_uri, &pkce.code_verifier).await?;
    Ok(tokens)
}

/// Try the primary loopback port first, fall back if busy.
async fn bind_loopback() -> Result<TcpListener> {
    match TcpListener::bind(("127.0.0.1", DEFAULT_PORT)).await {
        Ok(listener) => Ok(listener),
        Err(primary_err) => match TcpListener::bind(("127.0.0.1", FALLBACK_PORT)).await {
            Ok(listener) => Ok(listener),
            Err(fallback_err) => Err(Error::CodexLogin(format!(
                "could not bind callback server on :{DEFAULT_PORT} ({primary_err}) or :{FALLBACK_PORT} ({fallback_err})",
            ))),
        },
    }
}

pub(crate) fn build_authorize_url(
    issuer_base: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> Result<reqwest::Url> {
    reqwest::Url::parse_with_params(
        &format!("{issuer_base}{AUTHORIZE_PATH}"),
        &[
            ("response_type", "code"),
            ("client_id", CLIENT_ID),
            ("redirect_uri", redirect_uri),
            ("scope", SCOPES),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("state", state),
            ("originator", ORIGINATOR),
        ],
    )
    .map_err(|e| Error::CodexLogin(format!("could not build authorize URL: {e}")))
}

async fn await_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    tokio::time::timeout(CALLBACK_TIMEOUT, accept_callback(listener, expected_state))
        .await
        .map_err(|_| Error::CodexLogin("login timed out after 15 minutes".into()))?
}

/// Accept a single HTTP request, extract `code`+`state` from the query
/// string, write a success page, and close. Loops on bad/foreign requests
/// (favicon, browser pre-fetch) so a Chrome speculative GET does not eat the
/// real callback.
async fn accept_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let request_line = match read_request_line(&mut stream).await {
            Ok(line) => line,
            Err(_) => continue,
        };
        let path_and_query = match request_line.split_whitespace().nth(1) {
            Some(p) => p.to_string(),
            None => continue,
        };
        if !path_and_query.starts_with(CALLBACK_PATH) {
            let _ = write_response(&mut stream, 404, "Not found").await;
            continue;
        }

        let params = parse_query(&path_and_query);
        if let Some(err) = params
            .iter()
            .find_map(|(k, v)| (k == "error").then(|| v.clone()))
        {
            let _ = write_response(&mut stream, 400, "Login failed.").await;
            return Err(Error::CodexLogin(format!("authorize denied: {err}")));
        }
        let state = params
            .iter()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.clone());
        let code = params
            .iter()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.clone());

        match (state, code) {
            (Some(state), Some(code)) if state == expected_state => {
                let _ = write_response(
                    &mut stream,
                    200,
                    "Login successful — you can close this tab.",
                )
                .await;
                return Ok(code);
            }
            (Some(_), Some(_)) => {
                let _ = write_response(&mut stream, 400, "State mismatch — please retry.").await;
                return Err(Error::CodexLogin(
                    "OAuth state mismatch — possible CSRF or stale callback".into(),
                ));
            }
            _ => {
                let _ = write_response(&mut stream, 400, "Missing code or state.").await;
                continue;
            }
        }
    }
}

/// Read up to the first CRLF or 8KB. Single line is all we need.
async fn read_request_line(stream: &mut tokio::net::TcpStream) -> Result<String> {
    const MAX: usize = 8 * 1024;
    let mut buf = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        if buf.len() >= MAX {
            return Err(Error::CodexLogin("callback request line too long".into()));
        }
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            break;
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n") {
            buf.truncate(buf.len() - 2);
            break;
        }
    }
    String::from_utf8(buf).map_err(|e| Error::CodexLogin(format!("non-utf8 request line: {e}")))
}

async fn write_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    };
    let html = format!(
        "<!DOCTYPE html><html><body><p>{}</p></body></html>",
        html_escape(body)
    );
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn parse_query(path_and_query: &str) -> Vec<(String, String)> {
    let qs = match path_and_query.split_once('?') {
        Some((_, qs)) => qs,
        None => return Vec::new(),
    };
    qs.split('&')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            let k = percent_decode(k);
            let v = percent_decode(v);
            Some((k, v))
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'+' {
            out.push(' ');
            continue;
        }
        if b != b'%' {
            out.push(b as char);
            continue;
        }
        let h = bytes.next();
        let l = bytes.next();
        if let (Some(h), Some(l)) = (h, l) {
            if let (Some(hi), Some(lo)) = (hex(h), hex(l)) {
                out.push((hi * 16 + lo) as char);
                continue;
            }
        }
        out.push('%');
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct TokenResp {
    id_token: Option<String>,
    access_token: String,
    refresh_token: String,
}

pub(crate) async fn exchange_code(
    http: &reqwest::Client,
    issuer_base: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<ChatGptTokens> {
    let resp = http
        .post(format!("{issuer_base}{TOKEN_PATH}"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", CLIENT_ID),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::CodexLogin(format!(
            "token exchange failed: {status} {body}"
        )));
    }
    let parsed: TokenResp = resp.json().await?;
    let account_id = match parsed.id_token.as_deref() {
        Some(t) => jwt::chatgpt_account_id(t)?,
        None => None,
    };
    Ok(ChatGptTokens {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        id_token: parsed.id_token,
        account_id,
    })
}

fn open_url_best_effort(url: &str) {
    if cfg!(test) {
        return;
    }
    #[cfg(target_os = "linux")]
    let candidates: &[&str] = &["xdg-open"];
    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &["open"];
    #[cfg(target_os = "windows")]
    let candidates: &[&str] = &["cmd"];

    for cmd in candidates {
        let mut c = std::process::Command::new(cmd);
        #[cfg(target_os = "windows")]
        c.args(["/C", "start", "", url]);
        #[cfg(not(target_os = "windows"))]
        c.arg(url);
        if c.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn authorize_url_includes_required_params() {
        let url = build_authorize_url(
            "https://issuer.test",
            "http://localhost:1455/auth/callback",
            "challenge_xyz",
            "state_abc",
        )
        .unwrap();

        assert_eq!(url.host_str(), Some("issuer.test"));
        assert_eq!(url.path(), AUTHORIZE_PATH);

        let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs.get("client_id").map(String::as_str), Some(CLIENT_ID));
        assert_eq!(
            pairs.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(pairs.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(pairs.get("scope").map(String::as_str), Some(SCOPES));
        assert_eq!(pairs.get("state").map(String::as_str), Some("state_abc"));
        assert_eq!(
            pairs.get("redirect_uri").map(String::as_str),
            Some("http://localhost:1455/auth/callback"),
        );
        assert_eq!(
            pairs.get("originator").map(String::as_str),
            Some(ORIGINATOR)
        );
    }

    #[test]
    fn parse_query_handles_percent_encoding_and_plus() {
        let parsed = parse_query("/auth/callback?code=abc+def&state=hello%20world");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ("code".into(), "abc def".into()));
        assert_eq!(parsed[1], ("state".into(), "hello world".into()));
    }

    #[test]
    fn parse_query_returns_empty_when_no_question_mark() {
        assert!(parse_query("/auth/callback").is_empty());
    }

    fn fake_id_token() -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&serde_json::json!({
                "https://api.openai.com/auth": { "chatgpt_account_id": "acct_xyz" },
            }))
            .unwrap(),
        );
        let sig = URL_SAFE_NO_PAD.encode(b"sig");
        format!("{header}.{payload}.{sig}")
    }

    #[tokio::test]
    async fn exchange_code_parses_tokens_and_account_id() {
        let server = MockServer::start().await;
        let id_token = fake_id_token();
        Mock::given(method("POST"))
            .and(path(TOKEN_PATH))
            .and(header("content-type", "application/x-www-form-urlencoded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id_token": id_token,
                "access_token": "at_xxx",
                "refresh_token": "rt_xxx",
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let tokens = exchange_code(
            &http,
            &server.uri(),
            "thecode",
            "http://localhost:1455/auth/callback",
            "verifier",
        )
        .await
        .unwrap();

        assert_eq!(tokens.access_token, "at_xxx");
        assert_eq!(tokens.refresh_token, "rt_xxx");
        assert_eq!(tokens.account_id.as_deref(), Some("acct_xyz"));
    }

    #[tokio::test]
    async fn exchange_code_surfaces_4xx_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(TOKEN_PATH))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let err = exchange_code(
            &http,
            &server.uri(),
            "bad",
            "http://localhost:1455/auth/callback",
            "v",
        )
        .await
        .unwrap_err();
        match err {
            Error::CodexLogin(msg) => assert!(msg.contains("invalid_grant"), "got {msg}"),
            other => panic!("expected CodexLogin, got {other:?}"),
        }
    }
}

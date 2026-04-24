use std::time::Duration;

use serde::Deserialize;

use crate::error::{Error, Result};

/// Public GitHub OAuth client_id shipped by VS Code Copilot. The device flow
/// does not require a client secret for this client.
pub const VSCODE_COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

pub const GITHUB_BASE: &str = "https://github.com";

#[derive(Debug, Deserialize)]
struct DeviceCodeResp {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[allow(dead_code)]
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TokenResp {
    Ok { access_token: String },
    Err { error: String },
}

/// Run the full device flow and return a GitHub OAuth access token.
pub async fn run(http: &reqwest::Client, base_url: &str, client_id: &str) -> Result<String> {
    let code = request_code(http, base_url, client_id).await?;
    eprintln!(
        "Open {url} in your browser and enter the code: {user_code}",
        url = code.verification_uri,
        user_code = code.user_code,
    );
    open_url_best_effort(&code.verification_uri);
    poll_token(http, base_url, client_id, &code.device_code, code.interval).await
}

async fn request_code(
    http: &reqwest::Client,
    base_url: &str,
    client_id: &str,
) -> Result<DeviceCodeResp> {
    let resp = http
        .post(format!("{base_url}/login/device/code"))
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "read:user")])
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::DeviceFlow(format!(
            "device/code request failed: {status} {body}"
        )));
    }
    Ok(resp.json().await?)
}

async fn poll_token(
    http: &reqwest::Client,
    base_url: &str,
    client_id: &str,
    device_code: &str,
    initial_interval: u64,
) -> Result<String> {
    let mut interval = initial_interval.max(1);
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let resp = http
            .post(format!("{base_url}/login/oauth/access_token"))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;
        let parsed: TokenResp = resp.json().await?;
        match parsed {
            TokenResp::Ok { access_token } => return Ok(access_token),
            TokenResp::Err { error } => match error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += 5;
                    continue;
                }
                "access_denied" => {
                    return Err(Error::DeviceFlow("user denied the request".into()));
                }
                "expired_token" => {
                    return Err(Error::DeviceFlow(
                        "device code expired — please re-run login".into(),
                    ));
                }
                other => return Err(Error::DeviceFlow(format!("server error: {other}"))),
            },
        }
    }
}

fn open_url_best_effort(url: &str) {
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn full_flow_with_slow_down_then_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "DCODE",
                "user_code": "ABCD-EFGH",
                "verification_uri": "https://example.test/device",
                "expires_in": 900,
                "interval": 1,
            })))
            .mount(&server)
            .await;

        let responses = [
            serde_json::json!({ "error": "authorization_pending" }),
            serde_json::json!({ "error": "slow_down" }),
            serde_json::json!({ "access_token": "gho_real" }),
        ];
        for body in responses {
            Mock::given(method("POST"))
                .and(path("/login/oauth/access_token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .up_to_n_times(1)
                .mount(&server)
                .await;
        }

        let http = reqwest::Client::new();
        let token = run(&http, &server.uri(), "test-client").await.unwrap();
        assert_eq!(token, "gho_real");
    }

    #[tokio::test]
    async fn access_denied_surfaces_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "DCODE",
                "user_code": "ABCD-EFGH",
                "verification_uri": "https://example.test/device",
                "expires_in": 900,
                "interval": 1,
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "error": "access_denied" })),
            )
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let err = run(&http, &server.uri(), "test-client").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("denied"), "got {msg}");
    }
}

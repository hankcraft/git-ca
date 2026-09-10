use serde::Deserialize;

use super::client::{map_error, Client};
use crate::error::{Error, Result};

pub fn installed_client_version() -> Result<String> {
    let output = std::process::Command::new("codex")
        .arg("--version")
        .output()
        .map_err(|e| {
            Error::Config(format!(
                "cannot run `codex --version`: {e}; ensure Codex CLI is on PATH"
            ))
        })?;
    if !output.status.success() {
        return Err(Error::Config(format!(
            "`codex --version` failed: {}",
            output.status
        )));
    }
    parse_client_version(&String::from_utf8_lossy(&output.stdout))
}

fn parse_client_version(output: &str) -> Result<String> {
    let mut fields = output.split_whitespace();
    if fields.next() == Some("codex-cli") {
        if let Some(version) = fields.next().filter(|_| fields.next().is_none()) {
            // The backend expects a whole Codex version, including for prereleases.
            let version = version.split(['-', '+']).next().unwrap_or_default();
            let parts: Vec<_> = version.split('.').collect();
            if parts.len() == 3
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            {
                return Ok(version.to_string());
            }
        }
    }
    Err(Error::Config(
        "unrecognized `codex --version` output; expected `codex-cli MAJOR.MINOR.PATCH`".into(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct Model {
    pub slug: String,
    pub display_name: String,
    visibility: String,
    priority: i64,
}

#[derive(Debug, Deserialize)]
struct ModelsResp {
    models: Vec<Model>,
}

impl Client {
    pub async fn list_chat_models(&self, client_version: &str) -> Result<Vec<Model>> {
        let resp = self
            .http()
            .get(format!("{}/models", self.base_url()))
            .query(&[("client_version", client_version)])
            .headers(self.headers())
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(map_error(resp).await);
        }
        let mut parsed: ModelsResp = resp.json().await?;
        // ChatGPT OAuth can use models even when supported_in_api is false.
        parsed.models.retain(|model| model.visibility == "list");
        parsed.models.sort_by_key(|model| model.priority);
        Ok(parsed.models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parses_installed_codex_versions_and_rejects_unexpected_output() {
        for (output, expected) in [
            ("codex-cli 0.154.0\n", "0.154.0"),
            ("codex-cli 1.200.3-alpha.4+build.5\r\n", "1.200.3"),
            ("codex-cli 1.2.3+build.5", "1.2.3"),
        ] {
            assert_eq!(parse_client_version(output).unwrap(), expected);
        }
        for output in [
            "",
            "git-ca 0.2.5",
            "codex-cli",
            "codex-cli 1.2",
            "codex-cli 1.x.3",
            "codex-cli 1..3",
            "codex-cli 1.2.3 extra",
        ] {
            assert!(parse_client_version(output).is_err(), "{output}");
        }
    }

    #[tokio::test]
    async fn discovery_uses_codex_version_and_account_catalog_visibility_and_priority() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            // The backend gates models on Codex versions, not git-ca releases.
            .and(query_param("client_version", "1.200.3"))
            .and(header("User-Agent", concat!("git-ca/", env!("CARGO_PKG_VERSION"))))
            .and(header("Authorization", "Bearer at_test"))
            .and(header("ChatGPT-Account-ID", "acct_test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [
                    {"slug": "later", "display_name": "Later", "visibility": "list", "priority": 9},
                    {"slug": "hidden", "display_name": "Hidden", "visibility": "hide", "priority": 0},
                    {"slug": "oauth", "display_name": "OAuth model", "visibility": "list", "priority": 1,
                     "supported_in_api": false, "supported_reasoning_levels": [{"effort": "high", "description": "Deep"}]}
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = Client::with_base(
            reqwest::Client::new(),
            "at_test",
            Some("acct_test"),
            server.uri(),
        );
        let version = parse_client_version("codex-cli 1.200.3-alpha.4").unwrap();
        let models = client.list_chat_models(&version).await.unwrap();
        assert_eq!(
            models.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(),
            ["oauth", "later"]
        );
        assert_eq!(models[0].display_name, "OAuth model");
    }

    #[tokio::test]
    async fn discovery_preserves_empty_catalog_and_auth_errors() {
        let server = MockServer::start().await;
        let client = Client::with_base(reqwest::Client::new(), "at", None, server.uri());
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"models": []})),
            )
            .mount(&server)
            .await;
        assert!(client.list_chat_models("1.200.3").await.unwrap().is_empty());
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        assert!(matches!(
            client.list_chat_models("1.200.3").await,
            Err(Error::CodexAuth)
        ));
    }
}

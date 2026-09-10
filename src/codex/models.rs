use serde::Deserialize;

use super::client::{map_error, Client};
use crate::error::Result;

// /models gates availability on Codex CLI versions, not git-ca's release version.
// Verified against Codex 0.154.0; update when tracking newer backend capabilities.
const CODEX_CLIENT_VERSION: &str = "0.154.0";

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
    pub async fn list_chat_models(&self) -> Result<Vec<Model>> {
        let resp = self
            .http()
            .get(format!("{}/models", self.base_url()))
            .query(&[("client_version", CODEX_CLIENT_VERSION)])
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
    use crate::error::Error;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn discovery_uses_codex_version_and_account_catalog_visibility_and_priority() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            // The backend gates models on Codex versions, not git-ca releases.
            .and(query_param("client_version", "0.154.0"))
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
        let models = client.list_chat_models().await.unwrap();
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
        assert!(client.list_chat_models().await.unwrap().is_empty());
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        assert!(matches!(
            client.list_chat_models().await,
            Err(Error::CodexAuth)
        ));
    }
}

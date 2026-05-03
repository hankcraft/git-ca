use serde::{Deserialize, Serialize};

use super::client::{map_error, Client};
use crate::commit_msg::prompt::ChatMessage;
use crate::error::{Error, Result};

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMsg,
}

#[derive(Debug, Deserialize)]
struct ChoiceMsg {
    #[serde(default)]
    content: Option<String>,
}

impl Client {
    pub async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<String> {
        let req = ChatRequest {
            model,
            messages,
            temperature: 0.2,
            stream: false,
        };
        let url = format!("{}/chat/completions", self.base_url());
        let resp = self
            .http()
            .post(url)
            .headers(self.headers())
            .json(&req)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(map_error(resp).await);
        }
        let parsed: ChatResponse = resp.json().await?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or(Error::EmptyModelResponse)?;
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit_msg::prompt::ChatMessage;
    use crate::copilot::client::Client;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn chat_returns_first_choice() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("Authorization", "Bearer cop"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [
                    { "message": { "role": "assistant", "content": "hello" } }
                ]
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let client = Client::with_base(http, "cop", server.uri());
        let out = client
            .chat("gpt-4o", &[ChatMessage::user("hi")])
            .await
            .unwrap();
        assert_eq!(out, "hello");
    }

    #[tokio::test]
    async fn chat_maps_401_to_copilot_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let client = Client::with_base(http, "cop", server.uri());
        let err = client
            .chat("gpt-4o", &[ChatMessage::user("hi")])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::CopilotAuth), "got {err:?}");
    }

    #[tokio::test]
    async fn chat_maps_429_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "7"))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let client = Client::with_base(http, "cop", server.uri());
        let err = client
            .chat("gpt-4o", &[ChatMessage::user("hi")])
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::CopilotRateLimited { retry_after: 7 }),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn empty_content_maps_to_empty_model_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [ { "message": { "role": "assistant", "content": "" } } ]
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let client = Client::with_base(http, "cop", server.uri());
        let err = client
            .chat("gpt-4o", &[ChatMessage::user("hi")])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::EmptyModelResponse), "got {err:?}");
    }
}

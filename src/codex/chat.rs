use serde::{Deserialize, Serialize};

use super::client::{map_error, Client};
use super::sse::SseParser;
use crate::commit_msg::prompt::ChatMessage;
use crate::error::{Error, Result};

/// Default Codex model when neither `--model` nor a persisted default is set.
/// `gpt-5.5` matches the slug codex's own `~/.codex/config.toml` writes after
/// `codex login`, and is the slug a smoke test against the live ChatGPT
/// backend confirmed works for accounts with Codex access. The server returns
/// a clear 400 ("model is not supported") when the chosen slug is wrong, so a
/// stale default is recoverable via `git ca config set-model`.
pub const FALLBACK_MODEL: &str = "gpt-5.5";

#[derive(Debug, Serialize)]
struct ResponsesRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    instructions: &'a str,
    input: Vec<InputItem<'a>>,
    tools: [(); 0],
    tool_choice: &'static str,
    parallel_tool_calls: bool,
    stream: bool,
    store: bool,
    include: [(); 0],
    client_metadata: ClientMetadata<'a>,
}

#[derive(Debug, Serialize)]
struct InputItem<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    role: &'static str,
    content: Vec<InputContent<'a>>,
}

#[derive(Debug, Serialize)]
struct InputContent<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    text: &'a str,
}

#[derive(Debug, Serialize)]
struct ClientMetadata<'a> {
    #[serde(rename = "x-codex-installation-id")]
    installation_id: &'a str,
}

/// Subset of the SSE `data:` JSON we care about. Each event sent on
/// `/responses` carries a `type` field that mirrors the SSE `event:` line —
/// we read it from the body so the parser does not have to be byte-identical
/// with the SSE field.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta { delta: String },
    #[serde(rename = "response.completed")]
    Completed,
    #[serde(rename = "response.failed")]
    Failed {
        #[serde(default)]
        response: Option<FailedResponse>,
    },
    #[serde(rename = "response.incomplete")]
    Incomplete,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct FailedResponse {
    #[serde(default)]
    error: Option<FailedError>,
}

#[derive(Debug, Deserialize)]
struct FailedError {
    #[serde(default)]
    message: Option<String>,
}

impl Client {
    pub async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<String> {
        let (instructions, input) = build_input(messages);
        let req = ResponsesRequest {
            model,
            instructions: &instructions,
            input,
            tools: [],
            tool_choice: "auto",
            parallel_tool_calls: false,
            stream: true,
            store: false,
            include: [],
            client_metadata: ClientMetadata {
                installation_id: self.session_id(),
            },
        };
        let url = format!("{}/responses", self.base_url());
        let resp = self
            .http()
            .post(url)
            .headers(self.headers())
            .header("Accept", "text/event-stream")
            .json(&req)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(map_error(resp).await);
        }

        let mut parser = SseParser::new();
        let mut text = String::new();
        let mut completed = false;
        let mut response = resp;
        while let Some(chunk) = response.chunk().await? {
            parser.push(&chunk);
            while let Some(ev) = parser.next_event() {
                match parse_event(&ev.data) {
                    StreamEvent::OutputTextDelta { delta } => text.push_str(&delta),
                    StreamEvent::Completed => {
                        completed = true;
                    }
                    StreamEvent::Failed { response } => {
                        let msg = response
                            .and_then(|r| r.error)
                            .and_then(|e| e.message)
                            .unwrap_or_else(|| "codex stream reported failure".into());
                        return Err(Error::CodexServer {
                            status: 200,
                            body: msg,
                        });
                    }
                    StreamEvent::Incomplete => {
                        return Err(Error::CodexServer {
                            status: 200,
                            body: "codex stream ended incomplete".into(),
                        });
                    }
                    StreamEvent::Other => {}
                }
            }
            if completed {
                break;
            }
        }

        if !completed && text.is_empty() {
            return Err(Error::EmptyModelResponse);
        }
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            return Err(Error::EmptyModelResponse);
        }
        Ok(trimmed)
    }
}

fn parse_event(data: &str) -> StreamEvent {
    serde_json::from_str(data).unwrap_or(StreamEvent::Other)
}

/// Split the chat-style messages into Responses-API `instructions`+`input`.
///
/// The Responses API's canonical home for system prompts is `instructions`
/// at the top level. Multiple system messages concatenate so callers can keep
/// the existing `[system, user]` shape.
fn build_input<'a>(messages: &'a [ChatMessage]) -> (String, Vec<InputItem<'a>>) {
    let mut instructions = String::new();
    let mut input = Vec::new();
    for msg in messages {
        if msg.role == "system" {
            if !instructions.is_empty() {
                instructions.push_str("\n\n");
            }
            instructions.push_str(&msg.content);
            continue;
        }
        let role = match msg.role {
            "assistant" => "assistant",
            _ => "user",
        };
        let content_kind = if role == "assistant" {
            "output_text"
        } else {
            "input_text"
        };
        input.push(InputItem {
            kind: "message",
            role,
            content: vec![InputContent {
                kind: content_kind,
                text: &msg.content,
            }],
        });
    }
    (instructions, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::client::Client;
    use crate::commit_msg::prompt::ChatMessage;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sse(events: &[(&str, serde_json::Value)]) -> String {
        let mut out = String::new();
        for (event, data) in events {
            out.push_str("event: ");
            out.push_str(event);
            out.push('\n');
            out.push_str("data: ");
            out.push_str(&serde_json::to_string(data).unwrap());
            out.push_str("\n\n");
        }
        out
    }

    #[test]
    fn build_input_lifts_system_to_instructions() {
        let messages = vec![ChatMessage::system("be terse"), ChatMessage::user("hello")];
        let (instructions, input) = build_input(&messages);
        assert_eq!(instructions, "be terse");
        assert_eq!(input.len(), 1);
        assert_eq!(input[0].role, "user");
        assert_eq!(input[0].content[0].kind, "input_text");
        assert_eq!(input[0].content[0].text, "hello");
    }

    #[test]
    fn build_input_concatenates_multiple_system_messages() {
        let messages = vec![
            ChatMessage::system("rule one"),
            ChatMessage::system("rule two"),
            ChatMessage::user("go"),
        ];
        let (instructions, _input) = build_input(&messages);
        assert_eq!(instructions, "rule one\n\nrule two");
    }

    #[tokio::test]
    async fn chat_assembles_text_deltas_and_returns_on_completed() {
        let server = MockServer::start().await;
        let body = sse(&[
            (
                "response.created",
                serde_json::json!({"type":"response.created","response":{"id":"r1"}}),
            ),
            (
                "response.output_text.delta",
                serde_json::json!({"type":"response.output_text.delta","delta":"feat: "}),
            ),
            (
                "response.output_text.delta",
                serde_json::json!({"type":"response.output_text.delta","delta":"add x"}),
            ),
            (
                "response.completed",
                serde_json::json!({"type":"response.completed","response":{"id":"r1"}}),
            ),
        ]);
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("Authorization", "Bearer at_xxx"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let client = Client::with_base(http, "at_xxx", Some("acct_abc"), server.uri());
        let out = client
            .chat("gpt-5", &[ChatMessage::user("hi")])
            .await
            .unwrap();
        assert_eq!(out, "feat: add x");
    }

    #[tokio::test]
    async fn chat_maps_401_to_codex_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let client = Client::with_base(http, "at", None, server.uri());
        let err = client
            .chat("gpt-5", &[ChatMessage::user("hi")])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::CodexAuth), "got {err:?}");
    }

    #[tokio::test]
    async fn chat_maps_429_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "11"))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let client = Client::with_base(http, "at", None, server.uri());
        let err = client
            .chat("gpt-5", &[ChatMessage::user("hi")])
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::CodexRateLimited { retry_after: 11 }),
            "got {err:?}",
        );
    }

    #[tokio::test]
    async fn stream_failure_event_surfaces_codex_server_error() {
        let server = MockServer::start().await;
        let body = sse(&[(
            "response.failed",
            serde_json::json!({
                "type": "response.failed",
                "response": {
                    "error": { "message": "model unavailable" }
                },
            }),
        )]);
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let client = Client::with_base(http, "at", None, server.uri());
        let err = client
            .chat("gpt-5", &[ChatMessage::user("hi")])
            .await
            .unwrap_err();
        match err {
            Error::CodexServer { body, .. } => {
                assert!(body.contains("model unavailable"), "got {body}")
            }
            other => panic!("expected CodexServer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_stream_maps_to_empty_model_response() {
        let server = MockServer::start().await;
        let body = sse(&[(
            "response.completed",
            serde_json::json!({"type":"response.completed","response":{"id":"r1"}}),
        )]);
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let client = Client::with_base(http, "at", None, server.uri());
        let err = client
            .chat("gpt-5", &[ChatMessage::user("hi")])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::EmptyModelResponse), "got {err:?}");
    }
}

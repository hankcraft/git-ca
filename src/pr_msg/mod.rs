use serde::Deserialize;

use crate::error::{Error, Result};

pub mod prompt {
    use crate::cli::PrSource;
    use crate::commit_msg::prompt::ChatMessage;

    /// Anything past this gets truncated with a marker. Keep parity with commit
    /// drafting so PR generation handles large branches predictably.
    const SOURCE_CHAR_LIMIT: usize = 32_000;

    const SYSTEM_PROMPT: &str = "\
You are a senior engineer writing GitHub pull request text.

Respond with ONLY compact JSON — no prose before or after, no code fences, no
quoting outside JSON.

Required JSON shape:
{
  \"title\": \"short imperative PR title\",
  \"body\": \"Markdown PR body\"
}

Rules:
- title: imperative mood, no trailing period, ≤ 72 chars
- body: Markdown, include Summary and Testing sections
- explain user-visible behavior and review-relevant risks
- do not invent tickets, reviewers, labels, or test results not present in the input
";

    pub fn build(source: PrSource, base: &str, text: &str) -> Vec<ChatMessage> {
        let source_label = match source {
            PrSource::Diff => "Branch diff",
            PrSource::Commits => "Commit log",
        };
        vec![
            ChatMessage::system(SYSTEM_PROMPT),
            ChatMessage::user(format!(
                "Base branch: {base}\nSource: {source_label}\n\n```text\n{}\n```",
                truncate(text)
            )),
        ]
    }

    fn truncate(text: &str) -> String {
        if text.len() <= SOURCE_CHAR_LIMIT {
            return text.to_string();
        }
        let mut end = SOURCE_CHAR_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let mut out = String::with_capacity(end + 64);
        out.push_str(&text[..end]);
        out.push_str("\n\n# ... PR source truncated to ");
        out.push_str(&end.to_string());
        out.push_str(" bytes ...\n");
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn build_mentions_commit_log_source() {
            let messages = build(PrSource::Commits, "main", "feat: add PR flow");

            assert!(messages[1].content.contains("Base branch: main"));
            assert!(messages[1].content.contains("Source: Commit log"));
            assert!(messages[1].content.contains("feat: add PR flow"));
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PullRequestMessage {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Deserialize)]
struct RawPullRequestMessage {
    title: String,
    body: String,
}

pub fn parse_json(raw: &str) -> Result<PullRequestMessage> {
    let raw = crate::commit_msg::strip_code_fences(raw);
    let msg: RawPullRequestMessage = serde_json::from_str(&raw)?;
    let title = msg.title.trim().to_string();
    let body = msg.body.trim().to_string();
    if title.is_empty() {
        return Err(Error::Config("PR title cannot be empty".to_string()));
    }
    if body.is_empty() {
        return Err(Error::Config("PR body cannot be empty".to_string()));
    }
    Ok(PullRequestMessage { title, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pr_message_json() {
        let msg =
            parse_json(r###"{"title":"Add PR drafts","body":"## Summary\n- adds PR flow"}"###)
                .unwrap();

        assert_eq!(
            msg,
            PullRequestMessage {
                title: "Add PR drafts".to_string(),
                body: "## Summary\n- adds PR flow".to_string(),
            }
        );
    }

    #[test]
    fn parses_fenced_pr_message_json() {
        let msg = parse_json(
            "```json\n{\"title\":\"Add PR drafts\",\"body\":\"## Summary\\n- adds PR flow\"}\n```",
        )
        .unwrap();

        assert_eq!(msg.title, "Add PR drafts");
        assert!(msg.body.contains("adds PR flow"));
    }

    #[test]
    fn rejects_empty_pr_title() {
        let err = parse_json(r#"{"title":" ","body":"body"}"#).unwrap_err();

        assert!(matches!(err, Error::Config(msg) if msg == "PR title cannot be empty"));
    }
}

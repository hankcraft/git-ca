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
- body: Markdown. Treat it as a mini architecture note that helps reviewers
  answer: \"What am I reviewing, why does it matter, and where should I focus?\"
- a PR message is different from a commit message: a commit explains one
  change; a PR explains the whole story
- include these sections, omitting only sections that truly do not apply:
  ## Summary
  ## Context
  ## Changes
  ## Trade-offs / Risks
  ## Testing
  ## Screenshots / Demo
  ## References
- Summary: explain what this PR changes at a high level
- Context: explain why the PR exists and what problem it solves
- Changes: describe key implementation points, not every file changed
- Trade-offs / Risks: call out limitations, migration notes, review risks, and
  behavior that old clients/users keep
- Testing: list only verification shown by the input; if none is visible, say
  \"Not run (not shown in input)\"
- Screenshots / Demo: include for UI-visible changes when the input supports it
- References: include only issues, incidents, docs, or links present in input
- describe HOW when implementation is non-trivial, reviewers need guidance,
  there are architectural decisions/trade-offs, the diff is large, the PR
  touches multiple layers, or future development is affected
- Avoid HOW when it only repeats the file diff
- bad HOW: \"Changed `order.ts`; updated `api.ts`; added `utils.ts`\"
- good HOW: \"Centralizes retry handling in the request layer so individual API
  calls do not need their own retry logic\"
- do not invent tickets, reviewers, labels, or test results not present in the input
";

    pub fn build(
        source: PrSource,
        base: &str,
        text: &str,
        system_prompt: Option<&str>,
    ) -> Vec<ChatMessage> {
        let source_label = match source {
            PrSource::Diff => "Branch diff",
            PrSource::Commits => "Commit log",
        };
        vec![
            ChatMessage::system(system_prompt.unwrap_or(SYSTEM_PROMPT)),
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
            let messages = build(PrSource::Commits, "main", "feat: add PR flow", None);

            assert!(messages[1].content.contains("Base branch: main"));
            assert!(messages[1].content.contains("Source: Commit log"));
            assert!(messages[1].content.contains("feat: add PR flow"));
        }

        #[test]
        fn build_uses_system_prompt_override() {
            let messages = build(
                PrSource::Diff,
                "main",
                "diff --git a/x b/x",
                Some("custom PR prompt"),
            );

            assert_eq!(messages[0].content, "custom PR prompt");
            assert!(messages[1].content.contains("Source: Branch diff"));
        }

        #[test]
        fn system_prompt_requests_review_story_sections() {
            let messages = build(PrSource::Diff, "main", "diff --git a/x b/x", None);

            let system = &messages[0].content;
            for section in [
                "## Summary",
                "## Context",
                "## Changes",
                "## Trade-offs / Risks",
                "## Testing",
                "## Screenshots / Demo",
                "## References",
            ] {
                assert!(system.contains(section), "missing section: {section}");
            }
            assert!(system.contains("mini architecture note"));
            assert!(system.contains("Avoid HOW when it only repeats the file diff"));
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

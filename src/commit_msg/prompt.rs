use serde::Serialize;

/// Provider-agnostic chat message. Both the Copilot client (chat-completions)
/// and the Codex client (Responses API) accept the same `{role, content}`
/// pair — Codex internally lifts the system role into the top-level
/// `instructions` field.
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system",
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user",
            content: content.into(),
        }
    }
}

/// Anything past this gets truncated with a marker. 32k chars ≈ 8k tokens,
/// which is well under every Copilot chat model's context window while
/// leaving room for the system prompt and the generated message.
const DIFF_CHAR_LIMIT: usize = 32_000;

const SYSTEM_PROMPT_PREFIX: &str = "\
You are a senior engineer writing a git commit message for a staged diff.

Respond with ONLY the commit message — no prose before or after, no code
fences, no quoting. It will be piped verbatim into `git commit`.

Follow Conventional Commits strictly:

  <type>[optional scope]: <subject>

  [optional body WHY]
  [optional body WHAT]
  [optional body IMPACT]

";

const PREDEFINED_RULES: &str = "\
- type ∈ {feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert}
- subject: imperative mood, lowercase, no trailing period, ≤ 72 chars
- leave a blank line between subject and body
- body: wrap at ~72 cols; focus on WHY, not a line-by-line summary; describe concisely
- body WHY: explain the problem, trigger, or risk avoided by this change
- body WHAT: for non-trivial changes, summarize the high-level approach;
  use concise bullets when they make the approach easier to scan
- body IMPACT: mention production-relevant side effects such as performance,
  breaking changes, migrations, observability, or logging changes
- omit body entirely if the subject is self-explanatory
- do not invent requirements, tickets, or co-authors that aren't in the diff
";

pub fn build(diff: &str, custom_rules: Option<&str>) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(system_prompt(custom_rules)),
        ChatMessage::user(format!("Staged diff:\n\n```diff\n{}\n```", truncate(diff))),
    ]
}

fn system_prompt(custom_rules: Option<&str>) -> String {
    let rules = custom_rules.unwrap_or(PREDEFINED_RULES);
    format!("{SYSTEM_PROMPT_PREFIX}Rules:\n{rules}")
}

fn truncate(diff: &str) -> String {
    if diff.len() <= DIFF_CHAR_LIMIT {
        return diff.to_string();
    }
    // Walk back to the nearest UTF-8 char boundary so a multi-byte char
    // straddling DIFF_CHAR_LIMIT (e.g. CJK source, emoji in a comment) does
    // not panic the slice.
    let mut end = DIFF_CHAR_LIMIT;
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = String::with_capacity(end + 64);
    out.push_str(&diff[..end]);
    out.push_str("\n\n# ... diff truncated to ");
    out.push_str(&end.to_string());
    out.push_str(" bytes ...\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_diff_passes_through_unchanged() {
        let diff = "diff --git a/x b/x\n+new line\n";
        let msgs = build(diff, None);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "system");
        assert!(msgs[1].content.contains("+new line"));
    }

    #[test]
    fn build_uses_custom_rules_in_predefined_system_prompt() {
        let msgs = build(
            "diff --git a/x b/x\n+new line\n",
            Some("- custom commit rule\n"),
        );

        assert!(msgs[0]
            .content
            .contains("Follow Conventional Commits strictly"));
        assert!(msgs[0].content.contains("Rules:\n- custom commit rule\n"));
        assert!(!msgs[0].content.contains("- subject: imperative mood"));
        assert!(msgs[1].content.contains("+new line"));
    }

    #[test]
    fn build_uses_predefined_rules_by_default() {
        let msgs = build("diff --git a/x b/x\n+new line\n", None);

        assert!(msgs[0].content.contains("Rules:\n- type ∈"));
        assert!(msgs[0].content.contains("- subject: imperative mood"));
    }

    #[test]
    fn oversize_diff_gets_truncated_with_marker() {
        let diff: String = "+".repeat(DIFF_CHAR_LIMIT + 100);
        let msgs = build(&diff, None);
        assert!(msgs[1].content.contains("diff truncated"));
        // content length <= limit + marker slack
        assert!(msgs[1].content.len() < DIFF_CHAR_LIMIT + 500);
    }

    #[test]
    fn truncate_at_multibyte_char_boundary_does_not_panic() {
        // Place 'é' (2 bytes 0xC3 0xA9) so its first byte lands at
        // DIFF_CHAR_LIMIT - 1, guaranteeing a non-boundary cut at the limit.
        let mut s = "a".repeat(DIFF_CHAR_LIMIT - 1);
        s.push('é');
        s.push_str(&"x".repeat(100));
        assert!(s.len() > DIFF_CHAR_LIMIT);
        let msgs = build(&s, None);
        assert!(msgs[1].content.contains("diff truncated"));
    }
}

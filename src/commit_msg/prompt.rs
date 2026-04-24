use crate::copilot::ChatMessage;

/// Anything past this gets truncated with a marker. 32k chars ≈ 8k tokens,
/// which is well under every Copilot chat model's context window while
/// leaving room for the system prompt and the generated message.
const DIFF_CHAR_LIMIT: usize = 32_000;

const SYSTEM_PROMPT: &str = "\
You are a senior engineer writing a git commit message for a staged diff.

Respond with ONLY the commit message — no prose before or after, no code
fences, no quoting. It will be piped verbatim into `git commit`.

Follow Conventional Commits strictly:

  <type>[optional scope]: <subject>

  [optional body]

Rules:
- type ∈ {feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert}
- subject: imperative mood, lowercase, no trailing period, ≤ 72 chars
- leave a blank line between subject and body
- body: wrap at ~72 cols; explain WHY, not a line-by-line summary of the diff
- omit body entirely if the subject is self-explanatory
- do not invent requirements, tickets, or co-authors that aren't in the diff
";

pub fn build(diff: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(SYSTEM_PROMPT),
        ChatMessage::user(format!(
            "Staged diff:\n\n```diff\n{}\n```",
            truncate(diff)
        )),
    ]
}

fn truncate(diff: &str) -> String {
    if diff.len() <= DIFF_CHAR_LIMIT {
        return diff.to_string();
    }
    let mut out = String::with_capacity(DIFF_CHAR_LIMIT + 64);
    out.push_str(&diff[..DIFF_CHAR_LIMIT]);
    out.push_str("\n\n# ... diff truncated to ");
    out.push_str(&DIFF_CHAR_LIMIT.to_string());
    out.push_str(" chars ...\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_diff_passes_through_unchanged() {
        let diff = "diff --git a/x b/x\n+new line\n";
        let msgs = build(diff);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "system");
        assert!(msgs[1].content.contains("+new line"));
    }

    #[test]
    fn oversize_diff_gets_truncated_with_marker() {
        let diff: String = "+".repeat(DIFF_CHAR_LIMIT + 100);
        let msgs = build(&diff);
        assert!(msgs[1].content.contains("diff truncated"));
        // content length <= limit + marker slack
        assert!(msgs[1].content.len() < DIFF_CHAR_LIMIT + 500);
    }
}

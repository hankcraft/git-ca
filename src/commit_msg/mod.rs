pub mod prompt;

/// Default model used when neither `--model` nor `config.default_model` is
/// set, when the active provider is Copilot. Codex has its own default
/// (`crate::codex::FALLBACK_MODEL`) because the model-id namespaces differ.
pub const FALLBACK_MODEL: &str = "gpt-4o";

/// Some models wrap commit messages in ``` fences despite the system prompt.
/// Strip a single outer fence if present; leave inner backticks alone.
pub fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        // drop an optional language tag on the opening fence
        let after_lang = rest.split_once('\n').map(|(_, body)| body).unwrap_or(rest);
        if let Some(body) = after_lang.strip_suffix("```") {
            return body.trim().to_string();
        }
        if let Some(body) = after_lang.rsplit_once("\n```").map(|(b, _)| b) {
            return body.trim().to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::strip_code_fences;

    #[test]
    fn strips_triple_backtick_fence() {
        let s = "```\nfeat: x\n\nbody\n```";
        assert_eq!(strip_code_fences(s), "feat: x\n\nbody");
    }

    #[test]
    fn strips_fence_with_language_tag() {
        let s = "```text\nfix: y\n```";
        assert_eq!(strip_code_fences(s), "fix: y");
    }

    #[test]
    fn leaves_unfenced_message_alone() {
        let s = "refactor: z\n\nsome body";
        assert_eq!(strip_code_fences(s), s);
    }
}

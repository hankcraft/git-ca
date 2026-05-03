use std::path::PathBuf;
use std::process::Command;

use crate::error::{Error, Result};
use crate::pr_msg::PullRequestMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseBranch {
    pub pr_base: String,
    pub compare_ref: String,
}

impl BaseBranch {
    pub fn explicit(name: String) -> Self {
        Self {
            pr_base: name.clone(),
            compare_ref: name,
        }
    }
}

pub fn default_base() -> BaseBranch {
    let out = Command::new("git")
        .args([
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ])
        .output();
    let Ok(out) = out else {
        return fallback_base();
    };
    if !out.status.success() {
        return fallback_base();
    }
    base_from_origin_head(String::from_utf8_lossy(&out.stdout).trim())
}

fn fallback_base() -> BaseBranch {
    BaseBranch::explicit("main".to_string())
}

fn base_from_origin_head(branch: &str) -> BaseBranch {
    if let Some(pr_base) = branch.strip_prefix("origin/") {
        return BaseBranch {
            pr_base: pr_base.to_string(),
            compare_ref: branch.to_string(),
        };
    }
    BaseBranch::explicit(branch.to_string())
}

pub fn merge_base(base: &str) -> Result<String> {
    let out = super::run_git_capture(&["merge-base", base, "HEAD"])?;
    let hash = out.trim();
    if hash.is_empty() {
        return Err(Error::Config(format!(
            "unable to resolve merge-base for `{base}`"
        )));
    }
    Ok(hash.to_string())
}

pub fn branch_diff(base: &str) -> Result<String> {
    let args = diff_args(base);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = super::run_git_capture(&refs)?;
    non_empty_source(out, "branch has no changes against base")
}

pub fn commit_log(base: &str) -> Result<String> {
    let args = commit_log_args(base);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = super::run_git_capture(&refs)?;
    non_empty_source(out, "branch has no commits against base")
}

pub fn ensure_gh_available() -> Result<()> {
    let status = Command::new("gh").arg("--version").status()?;
    if status.success() {
        return Ok(());
    }
    Err(Error::Git(
        "gh --version".to_string(),
        status.code().unwrap_or(1),
    ))
}

pub fn edit_message(draft: &PullRequestMessage) -> Result<PullRequestMessage> {
    let path = message_path("PULL_REQUEST_EDITMSG")?;
    std::fs::write(&path, format!("{}\n\n{}\n", draft.title, draft.body))?;

    let editor = super::run_git_capture(&["var", "GIT_EDITOR"])?
        .trim()
        .to_string();
    if editor.is_empty() {
        return Err(Error::Config("GIT_EDITOR is empty".to_string()));
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} \"$1\"", editor))
        .arg("git-ca-editor")
        .arg(&path)
        .status()?;
    if !status.success() {
        return Err(Error::Git("editor".to_string(), status.code().unwrap_or(1)));
    }

    let edited = std::fs::read_to_string(&path)?;
    parse_editor_message(&edited)
}

pub fn create_pull_request(base: &str, title: &str, body: &str) -> Result<()> {
    let path = message_path("PULL_REQUEST_BODY")?;
    std::fs::write(&path, body)?;
    let body_file = path
        .to_str()
        .ok_or_else(|| Error::Config("PULL_REQUEST_BODY path is not UTF-8".into()))?;
    let args = gh_pr_create_args(base, title, body_file);
    let status = Command::new("gh").args(&args).status()?;
    if !status.success() {
        return Err(Error::Git(
            "gh pr create".to_string(),
            status.code().unwrap_or(1),
        ));
    }
    Ok(())
}

fn message_path(name: &str) -> Result<PathBuf> {
    let git_dir = super::run_git_capture(&["rev-parse", "--git-dir"])?
        .trim()
        .to_string();
    let mut path = PathBuf::from(git_dir);
    path.push(name);
    Ok(path)
}

fn non_empty_source(out: String, message: &str) -> Result<String> {
    if out.trim().is_empty() {
        return Err(Error::Config(message.to_string()));
    }
    Ok(out)
}

fn parse_editor_message(text: &str) -> Result<PullRequestMessage> {
    let trimmed = text.trim();
    let (title, body) = trimmed
        .split_once("\n\n")
        .ok_or_else(|| Error::Config("PR body cannot be empty".to_string()))?;
    let title = title.trim().to_string();
    let body = body.trim().to_string();
    if title.is_empty() {
        return Err(Error::Config("PR title cannot be empty".to_string()));
    }
    if body.is_empty() {
        return Err(Error::Config("PR body cannot be empty".to_string()));
    }
    Ok(PullRequestMessage { title, body })
}

pub(crate) fn diff_args(base: &str) -> Vec<String> {
    vec![
        "diff".to_string(),
        "--no-color".to_string(),
        "-U3".to_string(),
        format!("{base}...HEAD"),
    ]
}

pub(crate) fn commit_log_args(base: &str) -> Vec<String> {
    vec![
        "log".to_string(),
        "--no-merges".to_string(),
        "--format=%s%n%n%b".to_string(),
        format!("{base}..HEAD"),
    ]
}

pub(crate) fn gh_pr_create_args(base: &str, title: &str, body_file: &str) -> Vec<String> {
    vec![
        "pr".to_string(),
        "create".to_string(),
        "--base".to_string(),
        base.to_string(),
        "--title".to_string(),
        title.to_string(),
        "--body-file".to_string(),
        body_file.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_args_compare_base_to_head() {
        assert_eq!(
            diff_args("main"),
            ["diff", "--no-color", "-U3", "main...HEAD"]
        );
    }

    #[test]
    fn commit_log_args_compare_base_to_head() {
        assert_eq!(
            commit_log_args("main"),
            ["log", "--no-merges", "--format=%s%n%n%b", "main..HEAD"]
        );
    }

    #[test]
    fn gh_pr_create_args_include_title_and_body_file() {
        assert_eq!(
            gh_pr_create_args("main", "Add PR drafts", ".git/PULL_REQUEST_BODY"),
            [
                "pr",
                "create",
                "--base",
                "main",
                "--title",
                "Add PR drafts",
                "--body-file",
                ".git/PULL_REQUEST_BODY"
            ]
        );
    }

    #[test]
    fn default_origin_head_uses_remote_ref_for_comparison() {
        assert_eq!(
            base_from_origin_head("origin/main"),
            BaseBranch {
                pr_base: "main".to_string(),
                compare_ref: "origin/main".to_string(),
            }
        );
    }

    #[test]
    fn non_origin_default_base_uses_same_name_for_pr_and_comparison() {
        assert_eq!(
            base_from_origin_head("upstream/trunk"),
            BaseBranch::explicit("upstream/trunk".to_string())
        );
    }

    #[test]
    fn parse_editor_message_splits_title_and_body() {
        let msg = parse_editor_message("Add PR drafts\n\n## Summary\n- add flow\n").unwrap();

        assert_eq!(msg.title, "Add PR drafts");
        assert_eq!(msg.body, "## Summary\n- add flow");
    }
}

use std::path::PathBuf;
use std::process::Command;

use crate::error::{Error, Result};

/// Pre-fill `.git/COMMIT_EDITMSG` with `draft` and hand off to `git commit
/// -e -F <path>`. Git owns the editor lifecycle, so if the user wipes the
/// buffer and saves empty, git aborts the commit — we just propagate the
/// exit status.
pub fn commit_with_editor(draft: &str, no_verify: bool) -> Result<()> {
    commit_with_mode(draft, no_verify, true)
}

pub fn commit_generated(draft: &str, no_verify: bool) -> Result<()> {
    commit_with_mode(draft, no_verify, false)
}

fn commit_with_mode(draft: &str, no_verify: bool, edit: bool) -> Result<()> {
    let git_dir = super::run_git_capture(&["rev-parse", "--git-dir"])?
        .trim()
        .to_string();
    let mut msg_path = PathBuf::from(&git_dir);
    msg_path.push("COMMIT_EDITMSG");
    std::fs::write(&msg_path, draft)?;

    let path_str = msg_path
        .to_str()
        .ok_or_else(|| Error::Config("COMMIT_EDITMSG path is not UTF-8".into()))?
        .to_string();

    let mut cmd = Command::new("git");
    cmd.args(commit_args(&path_str, edit, no_verify));
    let status = cmd.status()?;
    if !status.success() {
        return Err(Error::Git("commit".into(), status.code().unwrap_or(1)));
    }
    Ok(())
}

fn commit_args(path: &str, edit: bool, no_verify: bool) -> Vec<String> {
    let mut args = vec!["commit".to_string()];
    if edit {
        args.push("-e".to_string());
    }
    args.extend(["-F".to_string(), path.to_string()]);
    if no_verify {
        args.push("--no-verify".to_string());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_commit_args_include_edit_flag() {
        let args = commit_args(".git/COMMIT_EDITMSG", true, false);

        assert_eq!(args, ["commit", "-e", "-F", ".git/COMMIT_EDITMSG"]);
    }

    #[test]
    fn generated_commit_args_omit_edit_flag() {
        let args = commit_args(".git/COMMIT_EDITMSG", false, false);

        assert_eq!(args, ["commit", "-F", ".git/COMMIT_EDITMSG"]);
    }

    #[test]
    fn no_verify_is_appended_to_generated_commit_args() {
        let args = commit_args(".git/COMMIT_EDITMSG", false, true);

        assert_eq!(args, ["commit", "-F", ".git/COMMIT_EDITMSG", "--no-verify"]);
    }
}

use std::path::PathBuf;
use std::process::Command;

use crate::error::{Error, Result};

/// Pre-fill `.git/COMMIT_EDITMSG` with `draft` and hand off to `git commit
/// -e -F <path>`. Git owns the editor lifecycle, so if the user wipes the
/// buffer and saves empty, git aborts the commit — we just propagate the
/// exit status.
pub fn commit_with_editor(draft: &str, no_verify: bool) -> Result<()> {
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
    cmd.args(["commit", "-e", "-F", &path_str]);
    if no_verify {
        cmd.arg("--no-verify");
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(Error::Git("commit".into(), status.code().unwrap_or(1)));
    }
    Ok(())
}

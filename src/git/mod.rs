pub mod commit;
pub mod diff;
pub mod pr;

use std::process::{Command, Output};

use crate::error::{Error, Result};

pub(crate) fn local_rules_file() -> Result<Option<std::path::PathBuf>> {
    let out = Command::new("git")
        .args(["config", "--local", "--get", "ca.rulesFile"])
        .output()
        .map_err(|e| Error::Config(format!("unable to read ca.rulesFile: {e}")))?;
    if out.status.code() == Some(1) && out.stderr.is_empty() {
        return Ok(None);
    }
    if !out.status.success() {
        return Err(Error::Config(format!(
            "unable to read ca.rulesFile (git exit {}): {}",
            out.status.code().unwrap_or(1),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let value = String::from_utf8(out.stdout)
        .map_err(|_| Error::Config("ca.rulesFile path is not UTF-8".into()))?;
    let value = value.strip_suffix('\n').unwrap_or(&value);
    if value.trim().is_empty() {
        return Err(Error::Config(
            "ca.rulesFile is empty or whitespace-only".into(),
        ));
    }
    Ok(Some(value.into()))
}

/// Run `git <args>` and capture stdout. Non-zero exit → Error::Git with
/// stderr piped through so users see the real message.
pub(crate) fn run_git_capture(args: &[&str]) -> Result<String> {
    let out: Output = Command::new("git").args(args).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let code = out.status.code().unwrap_or(1);
        let err = classify_git_failure(&args.join(" "), code, &stderr);
        if matches!(err, Error::NotGitRepository) {
            return Err(err);
        }
        eprint!("{stderr}");
        if !stderr.ends_with('\n') {
            eprintln!();
        }
        return Err(err);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn ensure_work_tree() -> Result<()> {
    let out = run_git_capture(&["rev-parse", "--is-inside-work-tree"])?;
    if out.trim() == "true" {
        return Ok(());
    }
    Err(Error::NotGitRepository)
}

fn classify_git_failure(command: &str, code: i32, stderr: &str) -> Error {
    if stderr.contains("not a git repository")
        || (command.starts_with("diff --cached") && code == 129)
    {
        return Error::NotGitRepository;
    }
    Error::Git(command.to_string(), code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_not_git_repository_stderr() {
        let err = classify_git_failure(
            "diff --cached --no-color -U3",
            128,
            "fatal: not a git repository (or any of the parent directories): .git",
        );

        assert!(matches!(err, Error::NotGitRepository));
    }

    #[test]
    fn classifies_no_index_cached_diff_as_not_git_repository() {
        let err = classify_git_failure(
            "diff --cached --no-color -U3",
            129,
            "error: unknown option `cached'\nusage: git diff --no-index [<options>] <path> <path>",
        );

        assert!(matches!(err, Error::NotGitRepository));
    }
}

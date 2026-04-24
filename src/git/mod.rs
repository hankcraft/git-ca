pub mod commit;
pub mod diff;

use std::process::{Command, Output};

use crate::error::{Error, Result};

/// Run `git <args>` and capture stdout. Non-zero exit → Error::Git with
/// stderr piped through so users see the real message.
pub(crate) fn run_git_capture(args: &[&str]) -> Result<String> {
    let out: Output = Command::new("git").args(args).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let code = out.status.code().unwrap_or(1);
        eprint!("{stderr}");
        if !stderr.ends_with('\n') {
            eprintln!();
        }
        return Err(Error::Git(args.join(" "), code));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

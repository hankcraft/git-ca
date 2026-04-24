use crate::error::{Error, Result};

/// Return the staged diff (`git diff --cached`). Empty output → the user has
/// nothing staged, which we surface as `Error::NoStagedChanges` so `main` can
/// print a friendly message and exit 1.
pub fn staged_diff() -> Result<String> {
    let out = super::run_git_capture(&["diff", "--cached", "--no-color", "-U3"])?;
    if out.trim().is_empty() {
        return Err(Error::NoStagedChanges);
    }
    Ok(out)
}

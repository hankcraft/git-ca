use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::{Error, Result};

/// Directory that holds config + auth files, e.g. `~/.config/git-ca`.
pub fn config_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "git-ca")
        .ok_or_else(|| Error::Config("unable to determine config directory".into()))?;
    Ok(dirs.config_dir().to_path_buf())
}

pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn auth_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("auth.json"))
}

/// Create the config directory if missing. On Unix, enforce mode 0700.
pub fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(&dir, perms)?;
    }
    Ok(dir)
}

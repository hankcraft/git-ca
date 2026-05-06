use std::path::PathBuf;

use crate::error::{Error, Result};

/// Directory that holds config + auth files, e.g. `~/.config/git-ca`.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(base).join("git-ca"));
    }

    let home = std::env::var_os("HOME")
        .ok_or_else(|| Error::Config("unable to determine home directory".into()))?;
    Ok(PathBuf::from(home).join(".config").join("git-ca"))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn auth_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("auth.json"))
}

pub fn commit_system_prompt_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("commit-system-prompt.md"))
}

pub fn pr_system_prompt_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("pr-system-prompt.md"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(key: &str, value: Option<&str>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn config_dir_uses_xdg_config_home_when_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        set_env("XDG_CONFIG_HOME", Some("/tmp/git-ca-xdg"));
        set_env("HOME", Some("/tmp/git-ca-home"));

        assert_eq!(
            config_dir().unwrap(),
            PathBuf::from("/tmp/git-ca-xdg/git-ca")
        );

        set_env(
            "XDG_CONFIG_HOME",
            old_xdg.as_deref().and_then(|v| v.to_str()),
        );
        set_env("HOME", old_home.as_deref().and_then(|v| v.to_str()));
    }

    #[test]
    fn config_dir_falls_back_to_home_dot_config() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        set_env("XDG_CONFIG_HOME", None);
        set_env("HOME", Some("/tmp/git-ca-home"));

        assert_eq!(
            config_dir().unwrap(),
            PathBuf::from("/tmp/git-ca-home/.config/git-ca")
        );

        set_env(
            "XDG_CONFIG_HOME",
            old_xdg.as_deref().and_then(|v| v.to_str()),
        );
        set_env("HOME", old_home.as_deref().and_then(|v| v.to_str()));
    }

    #[test]
    fn empty_xdg_config_home_falls_back_to_home_dot_config() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        set_env("XDG_CONFIG_HOME", Some(""));
        set_env("HOME", Some("/tmp/git-ca-home"));

        assert_eq!(
            config_dir().unwrap(),
            PathBuf::from("/tmp/git-ca-home/.config/git-ca")
        );

        set_env(
            "XDG_CONFIG_HOME",
            old_xdg.as_deref().and_then(|v| v.to_str()),
        );
        set_env("HOME", old_home.as_deref().and_then(|v| v.to_str()));
    }

    #[test]
    fn prompt_files_use_xdg_git_ca_config_dir_when_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        set_env("XDG_CONFIG_HOME", Some("/tmp/git-ca-xdg"));
        set_env("HOME", Some("/tmp/git-ca-home"));

        assert_eq!(
            commit_system_prompt_file().unwrap(),
            PathBuf::from("/tmp/git-ca-xdg/git-ca/commit-system-prompt.md")
        );
        assert_eq!(
            pr_system_prompt_file().unwrap(),
            PathBuf::from("/tmp/git-ca-xdg/git-ca/pr-system-prompt.md")
        );

        set_env(
            "XDG_CONFIG_HOME",
            old_xdg.as_deref().and_then(|v| v.to_str()),
        );
        set_env("HOME", old_home.as_deref().and_then(|v| v.to_str()));
    }

    #[test]
    fn prompt_files_use_home_git_ca_config_dir_when_xdg_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        set_env("XDG_CONFIG_HOME", None);
        set_env("HOME", Some("/tmp/git-ca-home"));

        assert_eq!(
            commit_system_prompt_file().unwrap(),
            PathBuf::from("/tmp/git-ca-home/.config/git-ca/commit-system-prompt.md")
        );
        assert_eq!(
            pr_system_prompt_file().unwrap(),
            PathBuf::from("/tmp/git-ca-home/.config/git-ca/pr-system-prompt.md")
        );

        set_env(
            "XDG_CONFIG_HOME",
            old_xdg.as_deref().and_then(|v| v.to_str()),
        );
        set_env("HOME", old_home.as_deref().and_then(|v| v.to_str()));
    }
}

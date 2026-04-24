pub mod paths;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// User preferences persisted to `config.json`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default model id used when `--model` is not passed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;
        read_json_or_default(&path)
    }

    pub fn save(&self) -> Result<()> {
        paths::ensure_config_dir()?;
        write_json_0600(&paths::config_file()?, self)
    }
}

pub(crate) fn read_json_or_default<T: serde::de::DeserializeOwned + Default>(
    path: &Path,
) -> Result<T> {
    match std::fs::read(path) {
        Ok(bytes) if bytes.is_empty() => Ok(T::default()),
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn write_json_0600<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    use std::io::Write;

    let tmp = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_file(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("git-ca-test-{}-{}", std::process::id(), name));
        p
    }

    #[test]
    fn round_trip_config() {
        let path = tmp_file("config.json");
        let _ = std::fs::remove_file(&path);
        let cfg = Config { default_model: Some("gpt-4o".into()) };
        write_json_0600(&path, &cfg).unwrap();

        let loaded: Config = read_json_or_default(&path).unwrap();
        assert_eq!(loaded.default_model.as_deref(), Some("gpt-4o"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "file should be chmod 0600");
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_file_returns_default() {
        let path = tmp_file("missing.json");
        let _ = std::fs::remove_file(&path);
        let cfg: Config = read_json_or_default(&path).unwrap();
        assert!(cfg.default_model.is_none());
    }
}

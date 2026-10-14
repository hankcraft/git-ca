use std::{fs, process::Command};

#[test]
fn config_defaults_round_trip_and_reset_without_persisting_flags() {
    let root = std::env::temp_dir().join(format!("git-ca-inference-config-{}", std::process::id()));
    let config = root.join("git-ca/config.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        r#"{"default_model":"hidden-model","auto_accept":true}"#,
    )
    .unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_git-ca"))
            .args(args)
            .env("XDG_CONFIG_HOME", &root)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    for key in ["fast", "reasoning"] {
        assert_eq!(
            run(&["config", &format!("get-codex-{key}")]),
            "default (backend)\n"
        );
    }
    run(&["config", "set-codex-fast", "true"]);
    assert_eq!(run(&["config", "get-codex-fast"]), "true\n");
    run(&["config", "set-codex-fast", "false"]);
    assert_eq!(run(&["config", "get-codex-fast"]), "false\n");
    for effort in ["low", "medium", "high", "xhigh"] {
        run(&["config", "set-codex-reasoning", effort]);
        assert_eq!(
            run(&["config", "get-codex-reasoning"]),
            format!("{effort}\n")
        );
    }
    let before = fs::read(&config).unwrap();
    let list = run(&["--fast", "--reasoning", "low", "config", "list"]);
    assert!(list.contains("codex_fast: false\ncodex_reasoning: xhigh\n"));
    assert_eq!(fs::read(&config).unwrap(), before);
    run(&["config", "set-codex-reasoning", "default"]);
    assert_eq!(
        run(&["config", "get-codex-reasoning"]),
        "default (backend)\n"
    );
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    assert!(saved.get("codex_reasoning").is_none());
    assert_eq!(saved["default_model"], "hidden-model");
    assert_eq!(saved["auto_accept"], true);
    assert_eq!(saved["codex_fast"], false);
    let before = fs::read(&config).unwrap();
    for args in [
        vec!["auth", "status"],
        vec!["models"],
        vec!["config", "get-codex-fast"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_git-ca"))
            .args(args)
            .args(["--fast=false", "--reasoning", "high"])
            .env("XDG_CONFIG_HOME", &root)
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            !stderr.contains("Codex-only") && !stderr.contains("drafting"),
            "{stderr}"
        );
        assert_eq!(fs::read(&config).unwrap(), before);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&config).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    fs::remove_dir_all(root).unwrap();
}

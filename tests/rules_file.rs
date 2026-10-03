#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn rules_file_selection_fails_before_generation_or_mutation() {
    let root = std::env::temp_dir().join(format!("git-ca-rules-flow-{}", std::process::id()));
    fs::create_dir_all(root.join("repo/subdir")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    let config = root.join("config/git-ca");
    fs::create_dir_all(&config).unwrap();
    let log = root.join("commands.log");
    for (name, script) in [
        ("git", "case \"$1\" in\nrev-parse) if [ \"$2\" = --show-toplevel ]; then printf '%s\\n' \"$RULES_TEST_ROOT\"; else echo true; fi;;\nconfig) if [ \"$RULES_TEST_CONFIG_ERROR\" = 1 ]; then echo lookup-failed >&2; exit 1; fi; [ \"${RULES_TEST_CONFIG+x}\" ] || exit 1; printf '%s\\n' \"$RULES_TEST_CONFIG\";;\nmerge-base) echo abc;;\ndiff|log) echo source-evidence;;\n*) exit 99;;\nesac\n"),
        ("gh", "[ \"$1\" = --version ] || exit 99\n"),
        ("editor", "exit 99\n"),
    ] {
        let path = root.join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{name}'\" $*\" >> \"$RULES_TEST_LOG\"\n{script}")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run_configured = |args: &[&str], value: Option<&str>, failed: bool| {
        fs::write(&log, "").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_git-ca"));
        command
            .args(args)
            .current_dir(root.join("repo/subdir"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("HOME", &root)
            .env("PATH", root.join("bin"))
            .env("GIT_EDITOR", root.join("bin/editor"))
            .env("RULES_TEST_LOG", &log)
            .env("RULES_TEST_ROOT", root.join("repo"))
            .env("RULES_TEST_CONFIG_ERROR", if failed { "1" } else { "0" })
            .env_remove("RULES_TEST_CONFIG");
        if let Some(value) = value {
            command.env("RULES_TEST_CONFIG", value);
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        let calls = fs::read_to_string(&log).unwrap();
        // Only source preconditions may run; generation, editor and mutations must not.
        for call in calls.lines() {
            assert!(
                call == "git rev-parse --is-inside-work-tree"
                    || call == "git rev-parse --show-toplevel"
                    || call == "git config --local --get ca.rulesFile"
                    || call == "git merge-base main HEAD"
                    || call.starts_with("git diff ")
                    || call.starts_with("git log ")
                    || call == "gh --version",
                "unexpected side effect: {call}"
            );
        }
        assert!(!root.join("repo/subdir/.git/COMMIT_EDITMSG").exists());
        assert!(!root.join("repo/subdir/.git/PULL_REQUEST_EDITMSG").exists());
        assert!(!root.join("repo/subdir/.git/PULL_REQUEST_BODY").exists());
        (output.status.code(), stderr)
    };
    let run = |args: &[&str]| run_configured(args, None, false);
    let commands = [
        vec![],
        vec!["pr", "--base", "main", "--source", "diff"],
        vec!["pr", "--base", "main", "--source", "commits"],
    ];
    let defaults = ["commit-system-prompt.md", "pr-system-prompt.md"];
    for name in defaults {
        fs::write(config.join(name), "valid default").unwrap();
    }
    fs::create_dir(root.join("repo/subdir/directory")).unwrap();
    fs::write(root.join("repo/subdir/invalid-utf8"), [0xff]).unwrap();
    fs::write(root.join("repo/subdir/empty"), " \n\t").unwrap();
    fs::write(root.join("repo/subdir/zero-length"), "").unwrap();
    fs::write(root.join("repo/team rules.md"), " \n- configured rule\n\t").unwrap();
    fs::write(root.join("repo/subdir/override"), "override rule\n").unwrap();
    for command in &commands {
        for path in [
            "missing",
            "directory",
            "invalid-utf8",
            "empty",
            "zero-length",
        ] {
            let value = format!("subdir/{path}");
            let (code, stderr) = run_configured(command, Some(&value), false);
            assert_eq!(code, Some(1), "{stderr}");
            assert!(
                stderr.contains("ca.rulesFile") && stderr.contains(path),
                "{stderr}"
            );
            assert!(!stderr.contains("valid default") && !stderr.contains("using built-in"));
        }
        for value in ["", " \n\t"] {
            let (code, stderr) = run_configured(command, Some(value), false);
            assert_eq!(code, Some(1), "{stderr}");
            assert!(stderr.contains("ca.rulesFile") && stderr.contains("empty"));
        }
        let (code, stderr) = run_configured(command, None, true);
        assert_eq!(code, Some(1), "{stderr}");
        assert!(stderr.contains("ca.rulesFile") && stderr.contains("lookup-failed"));
        for value in [
            "team rules.md".to_string(),
            root.join("repo/team rules.md").display().to_string(),
        ] {
            let (code, stderr) = run_configured(command, Some(&value), false);
            assert_eq!(code, Some(2), "{stderr}");
            assert!(!stderr.contains("using built-in"));
            assert_eq!(
                fs::read_to_string(&log)
                    .unwrap()
                    .contains("--show-toplevel"),
                !value.starts_with('/')
            );
        }
        let mut args = command.clone();
        args.extend(["--rules-file", "override"]);
        let (code, stderr) = run_configured(&args, Some("missing"), true);
        assert_eq!(code, Some(2), "{stderr}");
        assert!(!fs::read_to_string(&log).unwrap().contains("git config"));
    }
    for command in &commands {
        for path in ["missing", "directory", "invalid-utf8", "empty"] {
            let mut args = command.clone();
            args.extend(["--rules-file", path, "-y", "-n"]);
            let (code, stderr) = run(&args);
            assert_eq!(code, Some(1), "{stderr}");
            assert!(
                stderr.contains(path) && stderr.contains("rules file"),
                "{stderr}"
            );
            assert!(!stderr.contains("valid default"));
            assert!(!stderr.contains("drafting ") && !stderr.contains("using built-in"));
        }
    }
    for command in &commands {
        let own = defaults[usize::from(!command.is_empty())];
        let other = defaults[usize::from(command.is_empty())];
        for content in [
            Some(b"command rules\n".to_vec()),
            None,
            Some(vec![0xff]),
            Some(b" \n".to_vec()),
        ] {
            fs::write(config.join(other), [0xff]).unwrap();
            let warn = content
                .as_ref()
                .is_some_and(|bytes| bytes == &[0xff] || bytes == b" \n");
            if let Some(content) = content {
                fs::write(config.join(own), content).unwrap();
            } else {
                fs::remove_file(config.join(own)).unwrap();
            }
            let (code, stderr) = run(command);
            // No credentials: reaching authentication proves file resolution succeeded.
            assert_eq!(code, Some(2), "{stderr}");
            assert_eq!(stderr.contains("using built-in prompt"), warn, "{stderr}");
            assert!(!stderr.contains(other), "wrong command's default: {stderr}");
        }
    }
    for path in ["-", "~literal", "${RULES}", "../relative.rules"] {
        fs::write(root.join("repo/subdir").join(path), "rules\n").unwrap();
        for command in &commands {
            let mut args = command.clone();
            args.extend(["--rules-file", path]);
            let (code, stderr) = run(&args);
            assert_eq!(code, Some(2), "{stderr}");
            assert!(!stderr.contains("using built-in prompt"), "{stderr}");
        }
    }
    let absolute = root.join("absolute.rules");
    fs::write(&absolute, "rules\n").unwrap();
    assert_eq!(
        run(&["--rules-file", absolute.to_str().unwrap()]).0,
        Some(2)
    );
    for command in [
        vec!["config", "list"],
        vec!["auth", "status"],
        vec!["models"],
    ] {
        let (_, stderr) = run_configured(&command, Some("missing"), true);
        assert!(!stderr.contains("ca.rulesFile"), "{stderr}");
        assert!(fs::read_to_string(&log).unwrap().is_empty());
        let mut args = command;
        args.extend(["--rules-file", "missing"]);
        let (_, stderr) = run_configured(&args, Some("missing"), true);
        assert!(!stderr.contains("rules file"), "{stderr}");
        assert!(fs::read_to_string(&log).unwrap().is_empty());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_rules_use_isolated_git_config_and_each_worktree_root() {
    let root = std::env::temp_dir().join(format!("git-ca-real-rules-{}", std::process::id()));
    let repo = root.join("repo with spaces ");
    let linked = root.join("linked worktree ");
    let config = root.join("config/git-ca");
    fs::create_dir_all(repo.join("subdir")).unwrap();
    fs::create_dir_all(&config).unwrap();
    for name in ["global", "system"] {
        fs::write(root.join(name), "[ca]\n rulesFile = missing-scope-rules\n").unwrap();
    }
    fs::write(config.join("commit-system-prompt.md"), " \n").unwrap();
    let isolated = |program: &str, cwd: &std::path::Path| {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", &root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("GIT_CONFIG_GLOBAL", root.join("global"))
            .env("GIT_CONFIG_SYSTEM", root.join("system"))
            .env("GIT_TERMINAL_PROMPT", "0")
            .current_dir(cwd);
        command
    };
    let git = |cwd: &std::path::Path, args: &[&str]| {
        let output = isolated("git", cwd).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let run = |cwd: &std::path::Path, args: &[&str]| {
        let output = isolated(env!("CARGO_BIN_EXE_git-ca"), cwd)
            .args(args)
            .output()
            .unwrap();
        (
            output.status.code(),
            String::from_utf8(output.stderr).unwrap(),
        )
    };
    git(&repo, &["init", "--template="]);
    fs::write(repo.join("source"), "source evidence\n").unwrap();
    git(&repo, &["add", "source"]);
    let subdir = repo.join("subdir");
    let (code, stderr) = run(&subdir, &[]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(
        stderr.contains("using built-in prompt"),
        "global/system rules must be ignored: {stderr}"
    );

    let rules_name = " team rules.md ";
    fs::write(repo.join(rules_name), " \n- repository rules\n\t").unwrap();
    // A subdirectory decoy catches accidental working-directory resolution.
    fs::write(subdir.join(rules_name), [0xff]).unwrap();
    git(
        &subdir,
        &["config", "--local", "ca.rulesFile", "missing-first-value"],
    );
    git(
        &subdir,
        &["config", "--local", "--add", "ca.rulesFile", rules_name],
    );
    let (code, stderr) = run(&subdir, &[]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(
        !stderr.contains("using built-in"),
        "last local value must win: {stderr}"
    );
    git(&repo, &["config", "--local", "--unset-all", "ca.rulesFile"]);
    let absolute = root.join("absolute rules");
    fs::write(&absolute, "absolute rules\n").unwrap();
    git(
        &repo,
        &[
            "config",
            "--local",
            "ca.rulesFile",
            absolute.to_str().unwrap(),
        ],
    );
    assert_eq!(run(&subdir, &[]).0, Some(2));

    fs::write(subdir.join("override"), "working-directory override\n").unwrap();
    for value in ["", " \t", "missing"] {
        git(&repo, &["config", "--local", "ca.rulesFile", value]);
        let (code, stderr) = run(&subdir, &[]);
        assert_eq!(code, Some(1), "{stderr}");
        assert!(stderr.contains("ca.rulesFile"), "{stderr}");
        assert!(!stderr.contains("using built-in"));
        assert_eq!(run(&subdir, &["--rules-file", "override"]).0, Some(2));
    }
    let unreadable = repo.join("unreadable");
    fs::write(&unreadable, "do not print file contents").unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o0)).unwrap();
    // procfs denies reads even when the suite runs as root.
    let unreadable_path = if fs::read(&unreadable).is_err() {
        unreadable.clone()
    } else {
        std::path::PathBuf::from("/proc/1/mem")
    };
    git(
        &repo,
        &[
            "config",
            "--local",
            "ca.rulesFile",
            unreadable_path.to_str().unwrap(),
        ],
    );
    let (code, stderr) = run(&subdir, &[]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains("ca.rulesFile") && stderr.contains(&unreadable_path.display().to_string())
    );
    assert!(!stderr.contains("do not print file contents"));
    fs::set_permissions(unreadable, fs::Permissions::from_mode(0o600)).unwrap();

    git(&repo, &["config", "--local", "ca.rulesFile", rules_name]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Rules Test",
            "-c",
            "user.email=rules@example.invalid",
            "commit",
            "-m",
            "test source",
        ],
    );
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "rules-test",
            linked.to_str().unwrap(),
        ],
    );
    fs::create_dir(linked.join("subdir")).unwrap();
    fs::write(linked.join(rules_name), "linked-worktree rules\n").unwrap();
    fs::write(repo.join(rules_name), [0xff]).unwrap();
    fs::write(linked.join("source"), "linked source evidence\n").unwrap();
    git(&linked, &["add", "source"]);
    let (code, stderr) = run(&linked.join("subdir"), &[]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(
        !stderr.contains("using built-in"),
        "linked root must win: {stderr}"
    );

    fs::write(repo.join(".git/config"), "[invalid config\n").unwrap();
    let (code, stderr) = run(&subdir, &[]);
    // The existing repository precondition encounters malformed config first.
    assert_eq!(code, Some(128), "{stderr}");
    assert!(stderr.contains("bad config"), "{stderr}");
    fs::remove_dir_all(root).unwrap();
}

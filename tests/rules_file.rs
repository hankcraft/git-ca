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
        ("git", "case \"$1\" in\nrev-parse) echo true;;\nmerge-base) echo abc;;\ndiff|log) echo source-evidence;;\n*) exit 99;;\nesac\n"),
        ("gh", "[ \"$1\" = --version ] || exit 99\n"),
        ("editor", "exit 99\n"),
    ] {
        let path = root.join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{name}'\" $*\" >> \"$RULES_TEST_LOG\"\n{script}")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run = |args: &[&str]| {
        fs::write(&log, "").unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_git-ca"))
            .args(args)
            .current_dir(root.join("repo/subdir"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("HOME", &root)
            .env("PATH", root.join("bin"))
            .env("GIT_EDITOR", root.join("bin/editor"))
            .env("RULES_TEST_LOG", &log)
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        let calls = fs::read_to_string(&log).unwrap();
        // Only source preconditions may run; generation, editor and mutations must not.
        for call in calls.lines() {
            assert!(
                call == "git rev-parse --is-inside-work-tree"
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
        let mut args = command;
        args.extend(["--rules-file", "missing"]);
        let (_, stderr) = run(&args);
        assert!(!stderr.contains("rules file"), "{stderr}");
        assert!(fs::read_to_string(&log).unwrap().is_empty());
    }
    fs::remove_dir_all(root).unwrap();
}

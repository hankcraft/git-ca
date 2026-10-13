use std::{fs, os::unix::fs::PermissionsExt, process::Command};

use serde_json::json;
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

use super::*;

#[tokio::test]
async fn drafting_paths_preserve_inference_editor_and_output_contracts() {
    // Child processes isolate config, endpoints, cwd and PATH from other tests.
    if let Ok(args) = std::env::var("GIT_CA_TEST_ARGS") {
        let args: Vec<String> = serde_json::from_str(&args).unwrap();
        let expected = std::env::var("GIT_CA_TEST_EXPECTED").unwrap();
        match Cli::try_parse_from(args) {
            Err(_) => assert_eq!(expected, "invalid"),
            Ok(cli) => match run(cli).await {
                Ok(()) => assert_eq!(expected, "ok"),
                Err(error) => assert!(error.to_string().contains(&expected), "{error}"),
            },
        }
        return;
    }
    let server = MockServer::start().await;
    let root =
        std::env::temp_dir().join(format!("git-ca-drafting-inference-{}", std::process::id()));
    let config = root.join("config/git-ca");
    fs::create_dir_all(root.join("repo/.git")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(&config).unwrap();
    let log = root.join("commands.log");
    for (name, script) in [
        (
            "git",
            r#"case "$1" in
rev-parse) if [ "$2" = --git-dir ]; then echo "$GIT_CA_TEST_REPO/.git"; else echo true; fi;;
config) exit 1;;
diff) echo diff-evidence;;
log) echo commits-evidence;;
merge-base) echo abc;;
symbolic-ref) echo feature;;
var) echo "$GIT_CA_TEST_EDITOR";;
commit|push) exit 0;;
*) exit 99;;
esac
"#,
        ),
        (
            "gh",
            r#"case "$1 $2" in
'--version ') exit 0;;
'pr view') printf '{"state":"%s"}' "$GIT_CA_TEST_PR_STATE";;
'pr create'|'pr edit') exit 0;;
*) exit 99;;
esac
"#,
        ),
        ("editor", "exit 0\n"),
    ] {
        let file = root.join("bin").join(name);
        fs::write(
            &file,
            format!("#!/bin/sh\nprintf '%s\\n' '{name}'\" $*\" >> \"$GIT_CA_TEST_LOG\"\n{script}"),
        )
        .unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(
        config.join("config.json"),
        json!({"codex_fast":true,"codex_reasoning":"high"}).to_string(),
    )
    .unwrap();
    let saved = fs::read(config.join("config.json")).unwrap();
    let codex_auth = json!({"active_account":"test","accounts":{"test":{"name":"test","credential":{"provider":"codex","tokens":{"access_token":"old","refresh_token":"refresh","account_id":"acct"}}}}});
    let copilot_auth = json!({"active_account":"test","accounts":{"test":{"name":"test","credential":{"provider":"copilot","github_token":"test"}}}});
    let launch = |args: Vec<&str>, expected: &str, state: &str| {
        fs::write(&log, "").unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "inference_tests::drafting_paths_preserve_inference_editor_and_output_contracts",
                "--nocapture",
            ])
            .current_dir(root.join("repo"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    root.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("GIT_CA_TEST_ARGS", serde_json::to_string(&args).unwrap())
            .env("GIT_CA_TEST_EXPECTED", expected)
            .env("GIT_CA_TEST_CODEX_BASE", server.uri())
            .env("GIT_CA_TEST_CODEX_ISSUER", server.uri())
            .env("GIT_CA_TEST_REPO", root.join("repo"))
            .env("GIT_CA_TEST_EDITOR", root.join("bin/editor"))
            .env("GIT_CA_TEST_LOG", &log)
            .env("GIT_CA_TEST_PR_STATE", state)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(config.join("config.json")).unwrap(), saved);
        fs::read_to_string(&log).unwrap()
    };
    let response = |pr: bool| {
        let text = if pr {
            r#"{"title":"feat: generated","body":"body evidence"}"#
        } else {
            "```text\nfeat: generated\n```"
        };
        ResponseTemplate::new(200).set_body_string(format!(
            "data: {}\n\ndata: {}\n\n",
            json!({"type":"response.output_text.delta","delta":text}),
            json!({"type":"response.completed"})
        ))
    };
    for source in [None, Some("diff"), Some("commits")] {
        for state in if source.is_none() {
            vec!["CLOSED"]
        } else {
            vec!["CLOSED", "OPEN"]
        } {
            for yes in [false, true] {
                server.reset().await;
                fs::write(config.join("auth.json"), codex_auth.to_string()).unwrap();
                Mock::given(method("POST"))
                    .and(path("/responses"))
                    .respond_with(response(source.is_some()))
                    .expect(1)
                    .mount(&server)
                    .await;
                let mut args = vec!["git-ca", "--model", "hidden-model"];
                if let Some(source) = source {
                    args.extend(["pr", "--base", "main", "--source", source]);
                }
                if yes {
                    args.extend(["--yes", "--fast=false", "--reasoning", "low"]);
                }
                let calls = launch(args, "ok", state);
                let request = server.received_requests().await.unwrap().pop().unwrap();
                let body: serde_json::Value = request.body_json().unwrap();
                assert_eq!(body["model"], "hidden-model");
                assert_eq!(
                    body["service_tier"],
                    if yes { "default" } else { "priority" }
                );
                assert_eq!(
                    body["reasoning"]["effort"],
                    if yes { "low" } else { "high" }
                );
                let evidence = if source == Some("commits") {
                    "commits-evidence"
                } else {
                    "diff-evidence"
                };
                assert!(body["input"][0]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains(evidence));
                let git_dir = root.join("repo/.git");
                if source.is_none() {
                    assert_eq!(
                        fs::read_to_string(git_dir.join("COMMIT_EDITMSG")).unwrap(),
                        "feat: generated"
                    );
                    assert_eq!(calls.contains("commit -e "), !yes);
                } else {
                    assert_eq!(calls.contains("editor "), !yes);
                    assert!(calls.contains(if state == "OPEN" {
                        "gh pr edit"
                    } else {
                        "gh pr create"
                    }));
                    assert!(calls.contains("--title feat: generated"));
                    assert_eq!(calls.contains("git push"), state != "OPEN");
                    assert_eq!(
                        fs::read_to_string(git_dir.join("PULL_REQUEST_BODY")).unwrap(),
                        "body evidence"
                    );
                }
                server.verify().await;
            }
        }
    }
    server.reset().await;
    fs::write(config.join("auth.json"), codex_auth.to_string()).unwrap();
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("Authorization", "Bearer old"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token":"new","refresh_token":"rotated"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("Authorization", "Bearer new"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .expect(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("Authorization", "Bearer new"))
        .respond_with(response(false))
        .expect(1)
        .with_priority(2)
        .mount(&server)
        .await;
    launch(
        vec!["git-ca", "--yes", "--fast=false", "--reasoning", "xhigh"],
        "ok",
        "CLOSED",
    );
    let requests = server.received_requests().await.unwrap();
    let bodies: Vec<serde_json::Value> = requests
        .iter()
        .filter(|request| request.url.path() == "/responses")
        .map(|request| request.body_json().unwrap())
        .collect();
    assert_eq!(bodies.len(), 3);
    for body in &bodies {
        assert_eq!(body["service_tier"], "default");
        assert_eq!(body["reasoning"]["effort"], "xhigh");
    }
    server.verify().await;
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(400).set_body_string("unsupported inference"))
        .expect(1)
        .mount(&server)
        .await;
    let calls = launch(
        vec!["git-ca", "--fast", "--reasoning", "high"],
        "unsupported inference",
        "CLOSED",
    );
    assert!(!calls.contains("git commit") && !calls.contains("editor "));
    server.verify().await;
    server.reset().await;
    fs::write(config.join("auth.json"), copilot_auth.to_string()).unwrap();
    for command in [
        vec!["git-ca"],
        vec!["git-ca", "pr", "--base", "main", "--source", "diff"],
        vec!["git-ca", "pr", "--base", "main", "--source", "commits"],
    ] {
        for flag in [
            vec!["--fast"],
            vec!["--fast=false"],
            vec!["--reasoning", "low"],
        ] {
            let mut args = command.clone();
            args.extend(flag);
            let calls = launch(args, "Codex-only", "OPEN");
            assert!(
                !calls.contains("git commit")
                    && !calls.contains("gh pr")
                    && !calls.contains("editor ")
            );
        }
        let mut args = command;
        args.extend(["--reasoning", "bogus"]);
        assert!(launch(args, "invalid", "OPEN").is_empty());
    }
    assert!(server.received_requests().await.unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

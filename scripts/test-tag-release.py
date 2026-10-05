#!/usr/bin/env python3
"""Exercise release eligibility and immutable tags against a disposable Git remote."""

import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

helper = Path(__file__).with_name("tag-release.py").resolve()
metadata = helper.with_name("release-metadata.py")
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    repo = root / "repo"
    repo.mkdir()

    def git(*args):
        return subprocess.check_output(["git", *args], cwd=repo, text=True, stderr=subprocess.DEVNULL).strip()

    git("init", "-q", "-b", "main")
    git("config", "core.hooksPath", "/dev/null")
    git("config", "user.name", "Test")
    git("config", "user.email", "test@example.com")
    subprocess.run(["git", "init", "-q", "--bare", str(root / "remote")], check=True)
    git("remote", "add", "origin", str(root / "remote"))
    man = repo / "docs/man/git-ca.1"
    man.parent.mkdir(parents=True)
    man.write_text('.TH GIT-CA 1 "2026-01-01" "git-ca 0.3.0" "User Commands"\n')
    (repo / "Cargo.toml").write_text('[package]\nname="git-ca"\nversion="0.3.0"\n')
    (repo / "Cargo.lock").write_text('[[package]]\nname="git-ca"\nversion="0.3.0"\n')
    git("add", ".")
    git("commit", "-qm", "chore: baseline")
    baseline = git("rev-parse", "HEAD")
    git("checkout", "-qb", "release-plz-test")
    (repo / "CHANGELOG.md").write_text("Release 0.3.0\n")
    git("add", ".")
    git("commit", "-qm", "chore: release v0.3.0")
    head = git("rev-parse", "HEAD")
    git("checkout", "-q", "main")
    git("merge", "--no-ff", "-qm", "Merge release PR", "release-plz-test")
    sha = git("rev-parse", "HEAD")
    assert sha != head, "The checked merge commit must differ from the release branch head"
    (repo / "release-metadata.py").write_bytes(metadata.read_bytes())
    # The real helper resolves metadata alongside itself.
    (repo / "tag-release.py").write_bytes(helper.read_bytes())
    stub = root / "bin"
    stub.mkdir()
    (stub / "gh").write_text('#!/bin/sh\ncat "$PR_FIXTURE"\n')
    (stub / "gh").chmod(0o755)
    fixture = root / "prs.json"
    env = {**os.environ, "PATH": f"{stub}:{os.environ['PATH']}", "PR_FIXTURE": str(fixture),
           "GITHUB_SHA": sha, "GITHUB_REPOSITORY": "owner/repo", "RELEASE_BOT": "release-app[bot]"}
    pr = {"merged_at": "2026-01-01", "merge_commit_sha": sha, "user": {"login": "release-app[bot]"},
          "base": {"ref": "main", "repo": {"full_name": "owner/repo"}},
          "head": {"ref": "release-plz-test", "repo": {"full_name": "owner/repo"}}}

    def run(prs):
        fixture.write_text(json.dumps([prs]))
        return subprocess.run([sys.executable, str(repo / "tag-release.py")], cwd=repo, env=env, capture_output=True)

    ineligible = []
    for key, value in [("merged_at", None), ("merge_commit_sha", head)]:
        invalid = copy.deepcopy(pr)
        invalid[key] = value
        ineligible.append(invalid)
    for section, key, value in [("user", "login", "human"), ("base", "ref", "dev"),
                                ("head", "ref", "ordinary"), ("head", "repo", {"full_name": "fork/repo"})]:
        invalid = copy.deepcopy(pr)
        invalid[section][key] = value
        ineligible.append(invalid)
    for prs in [[], *[[invalid] for invalid in ineligible]]:
        assert run(prs).returncode == 0
        assert not git("ls-remote", "--tags", "origin"), "Ordinary and unmerged PRs must not tag"
    assert run([pr, pr]).returncode != 0, "Ambiguous release merges must fail"
    env["GITHUB_SHA"] = head
    assert run([pr]).returncode != 0, "A checkout other than the checked SHA must fail"
    env["GITHUB_SHA"] = sha
    valid = man.read_bytes()
    man.write_bytes(valid.replace(b"0.3.0", b"0.2.6"))
    assert run([pr]).returncode != 0
    assert not git("ls-remote", "--tags", "origin"), "Bad metadata must prevent tagging"
    man.write_bytes(valid)
    assert run([pr]).returncode == 0
    assert git("ls-remote", "origin", "refs/tags/v0.3.0").split()[0] == sha
    assert run([pr]).returncode == 0, "Same-SHA reruns must succeed without moving tags"
    git("push", "-q", "origin", ":refs/tags/v0.3.0")
    git("tag", "-a", "v0.3.0", "-m", "existing release", sha)
    git("push", "-q", "origin", "refs/tags/v0.3.0")
    assert run([pr]).returncode == 0, "Annotated tags must compare their peeled commit"
    git("push", "-q", "origin", ":refs/tags/v0.3.0")
    git("push", "-q", "origin", f"{baseline}:refs/tags/v0.3.0")
    assert run([pr]).returncode != 0, "Conflicting targets must fail"
    assert git("ls-remote", "origin", "refs/tags/v0.3.0").split()[0] == baseline
print("release tag checks passed")

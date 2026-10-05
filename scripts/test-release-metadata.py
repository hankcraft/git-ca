#!/usr/bin/env python3
"""Release PR metadata must agree without rewriting unrelated manual content."""

import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap

helper = Path(__file__).with_name("release-metadata.py").resolve()
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    man = root / "docs/man/git-ca.1"
    man.parent.mkdir(parents=True)
    (root / "Cargo.toml").write_text('[package]\nname = "git-ca"\nversion = "0.3.0"\n')
    lock = root / "Cargo.lock"
    lock.write_text('[[package]]\nname = "git-ca"\nversion = "0.3.0"\n')
    old = '.TH GIT-CA 1 "2026-01-01" "git-ca 0.2.6" "User Commands"'
    body = '\n.SH NAME\ngit-ca — keep Unicode and trailing whitespace  \n'
    man.write_text(old + body)

    def run(*args):
        return subprocess.run([sys.executable, str(helper), *args], cwd=root, capture_output=True)

    before = man.read_bytes()
    assert run("--check").returncode != 0, "A stale manual must block release checks"
    assert man.read_bytes() == before, "Checks must never modify metadata"
    assert run().returncode == 0
    expected = f'.TH GIT-CA 1 "{datetime.date.today().isoformat()}" "git-ca 0.3.0" "User Commands"'
    assert man.read_text() == expected + body, "Only the header may change"
    assert run("--check").returncode == 0, "Synchronized release PR must pass"
    # A matching version must preserve its preparation date across reruns.
    stable = (old.replace("0.2.6", "0.3.0") + body).encode()
    man.write_bytes(stable)
    assert run().returncode == 0 and man.read_bytes() == stable
    for invalid in [body, expected + "\n" + expected + body, expected.replace("2026", "0000") + body]:
        man.write_text(invalid)
        before = man.read_bytes()
        assert run().returncode != 0, "Missing, duplicate, or invalid headers must fail"
        assert man.read_bytes() == before
    man.write_text(expected + body)
    lock.write_text(lock.read_text().replace("0.3.0", "0.2.6"))
    assert run("--check").returncode != 0, "A stale lockfile must block release checks"
    assert run().returncode != 0, "Preparation must not conceal a stale lockfile"

# Exercise the actual workflow against a local remote: metadata belongs in the
# release commit, and reruns must leave that commit untouched.
workflow = helper.parent.parent / ".github/workflows/release-plz.yml"
step = workflow.read_text().split("      - name: Synchronize release PR metadata\n", 1)[1]
commands = textwrap.dedent(step.split("        run: |\n", 1)[1])
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory) / "checkout"
    root.mkdir()
    remote = Path(directory) / "remote.git"
    branch = "release-plz-test"
    env = dict(os.environ, PR=json.dumps({"head_branch": branch}), RELEASE_BOT="release-test[bot]")
    env.update(GIT_AUTHOR_NAME="Release author", GIT_AUTHOR_EMAIL="author@example.com",
               GIT_COMMITTER_NAME="Release committer", GIT_COMMITTER_EMAIL="committer@example.com")

    def git(*args):
        return subprocess.check_output(["git", *args], cwd=root, env=env, stderr=subprocess.PIPE).decode().strip()

    git("init", "--bare", str(remote))
    git("init", "-b", branch)
    git("remote", "add", "origin", str(remote))
    (root / "README.md").write_text("Base commit\n")
    git("add", ".")
    git("commit", "-m", "Initial commit")
    (root / "scripts").mkdir()
    (root / "scripts/release-metadata.py").write_bytes(helper.read_bytes())
    (root / "Cargo.toml").write_text('[package]\nname = "git-ca"\nversion = "0.3.0"\n')
    (root / "Cargo.lock").write_text('[[package]]\nname = "git-ca"\nversion = "0.3.0"\n')
    man = root / "docs/man/git-ca.1"
    man.parent.mkdir(parents=True)
    man.write_text(old + body)
    git("add", ".")
    git("commit", "-m", "chore: release v0.3.0")
    git("push", "origin", branch)
    original = git("rev-parse", "HEAD")
    identity = git("show", "-s", "--format=%P%n%B%n%an <%ae>", "HEAD")

    def synchronize():
        subprocess.run(["bash", "-e", "-o", "pipefail", "-c", commands], cwd=root, env=env,
                       check=True, capture_output=True)

    synchronize()
    amended = git("rev-parse", "HEAD")
    assert amended != original and git("rev-list", "--count", "HEAD") == "2", "Metadata must amend, not append a commit"
    assert git("show", "-s", "--format=%P%n%B%n%an <%ae>", "HEAD") == identity, "Preserve release parent, message, and author"
    assert git("ls-remote", "origin", f"refs/heads/{branch}").split()[0] == amended, "Push the amended release commit"
    assert '"git-ca 0.3.0"' in man.read_text()
    synchronize()
    assert git("rev-parse", "HEAD") == amended, "A no-op rerun must not rewrite the release commit"
print("release metadata checks passed")

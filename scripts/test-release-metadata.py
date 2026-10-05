#!/usr/bin/env python3
"""Release PR metadata must agree without rewriting unrelated manual content."""

import datetime
from pathlib import Path
import subprocess
import sys
import tempfile

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
print("release metadata checks passed")

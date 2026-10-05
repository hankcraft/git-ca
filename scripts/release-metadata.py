#!/usr/bin/env python3
"""Synchronize or check release metadata; run from the repository root."""

import argparse
import datetime
from pathlib import Path
import re
import sys
import tomllib


def reconcile(check):
    package = tomllib.loads(Path("Cargo.toml").read_text())["package"]
    version = package["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version):
        raise ValueError("Cargo.toml: invalid package version")
    locked = tomllib.loads(Path("Cargo.lock").read_text())["package"]
    own = [p for p in locked if p["name"] == package["name"] and "source" not in p]
    if len(own) != 1 or own[0]["version"] != version:
        raise ValueError("Cargo.lock: package version disagrees with Cargo.toml")

    path = Path("docs/man/git-ca.1")
    content = path.read_bytes().decode("utf-8")
    headers = list(re.finditer(r"(?m)^\.TH\s+GIT-CA\b[^\n]*", content))
    if len(headers) != 1:
        raise ValueError(f"{path}: expected one .TH GIT-CA header, found {len(headers)}")
    header = headers[0]
    parsed = re.fullmatch(r'\.TH GIT-CA 1 "(\d{4}-\d{2}-\d{2})" "git-ca ([^"]+)" "User Commands"', header[0])
    if parsed is None:
        raise ValueError(f"{path}: malformed .TH GIT-CA header")
    datetime.date.fromisoformat(parsed[1])
    if parsed[2] != version:
        if check:
            raise ValueError(f"{path}: version {parsed[2]} disagrees with {version}")
        replacement = f'.TH GIT-CA 1 "{datetime.date.today().isoformat()}" "git-ca {version}" "User Commands"'
        path.write_bytes((content[:header.start()] + replacement + content[header.end():]).encode("utf-8"))
    return version


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Validate without writing files")
    args = parser.parse_args()
    try:
        print(reconcile(args.check))
    except (OSError, ValueError, KeyError) as error:
        sys.exit(f"release metadata: {error}")

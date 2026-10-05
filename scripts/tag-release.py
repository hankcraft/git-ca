#!/usr/bin/env python3
"""Tag only a merged release PR's checked event SHA; CI runs checks first."""

import json
import os
from pathlib import Path
import subprocess
import sys


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def tag_release():
    sha = os.environ["GITHUB_SHA"]
    repo = os.environ["GITHUB_REPOSITORY"]
    bot = os.environ["RELEASE_BOT"]
    if run("git", "rev-parse", "HEAD") != sha:
        raise ValueError("Checkout does not match the checked event SHA")
    pages = json.loads(run("gh", "api", "--paginate", "--slurp", f"repos/{repo}/commits/{sha}/pulls"))
    releases = [
        pr for page in pages for pr in page
        if pr["merged_at"] and pr["merge_commit_sha"] == sha
        and pr["base"]["ref"] == "main"
        and pr["base"]["repo"]["full_name"] == repo
        and pr["head"]["ref"].startswith("release-plz-")
        and pr["head"]["repo"] and pr["head"]["repo"]["full_name"] == repo
        and pr["user"]["login"] == bot
    ]
    if not releases:
        print("No merged release PR at this SHA; skipping tag")
        return
    if len(releases) != 1:
        raise ValueError("Multiple merged release PRs at this SHA")
    version = run(sys.executable, str(Path(__file__).with_name("release-metadata.py")), "--check")
    ref = f"refs/tags/v{version}"
    existing = subprocess.run(["git", "ls-remote", "--exit-code", "origin", ref], capture_output=True, text=True)
    if existing.returncode == 0:
        run("git", "fetch", "--no-tags", "origin", ref)
        if run("git", "rev-parse", "FETCH_HEAD^{commit}") != sha:
            raise ValueError(f"{ref} already identifies another commit")
        print(f"{ref} already identifies {sha}; unchanged")
    elif existing.returncode == 2:
        run("git", "push", "origin", f"{sha}:{ref}")
        print(f"Created {ref} at {sha}")
    else:
        raise ValueError(f"Cannot inspect {ref}: {existing.stderr.strip()}")


if __name__ == "__main__":
    try:
        tag_release()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        sys.exit(f"release tag: {error}")

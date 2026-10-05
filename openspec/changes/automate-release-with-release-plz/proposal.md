## Why

Version preparation currently uses cargo-release on a maintainer checkout, while cargo-dist already automates publishing. Automating release PRs and post-merge tags removes manual release preparation while preserving reviewed, tested releases and keeping one version-management tool.

## What Changes

- Replace cargo-release and `release.toml` with release-plz configuration and GitHub Actions orchestration targeting `main`.
- Maintain one release PR containing proposed version, lockfile, changelog, and synchronized man-page metadata; retain maintainer review and merge as the release decision.
- Require formatting, Clippy, tests, and metadata consistency before release; create tags only after the release PR merges and checks succeed for that commit.
- Use release-plz only for PR preparation; explicitly tag the checked merge or squash commit after verifying the merged App-authored release PR.
- Retain cargo-dist and its existing GitHub Release, crates.io, npm, and Homebrew publishing jobs.
- Document GitHub App authentication, required checks, default-branch cutover, migration, and recovery from failed publishing.

## Capabilities

### New Capabilities

- `release-automation`: Reviewed release PR preparation, synchronized metadata, checked post-merge tagging, and handoff to existing distribution.

### Modified Capabilities

None. The project currently has no main release specification.

## Impact

Affected areas: new release-plz configuration and workflow, CI checks, a small man-page metadata helper and runnable check, removal of `release.toml`, and `docs/development.md`. Existing `dist-workspace.toml`, generated release workflow, and channel publishers remain the distribution authority. Local `scripts/release-local.sh` remains an explicit maintainer fallback. GitHub settings and App secrets require maintainer setup; this change does not authorize remote branch deletion, publishing, or changes to CLI behavior.

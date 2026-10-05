## Purpose

Automate reviewed version preparation and checked release tagging while preserving the existing distribution channels and consistent package metadata.

## ADDED Requirements

### Requirement: Reviewed release preparation
The system SHALL maintain at most one open release PR targeting `main`, proposing a version from Conventional Commits and including package version, lockfile, and changelog updates. It MUST leave merging to a maintainer.

#### Scenario: Unreleased changes accumulate
- **WHEN** releasable changes merge into `main`
- **THEN** automation creates or updates the release PR without publishing packages

### Requirement: Consistent release metadata
The system SHALL synchronize the man-page header with the proposed package version and a valid release preparation date, preserve unrelated content, and fail when the expected header cannot be identified uniquely.

#### Scenario: Release PR metadata is prepared
- **WHEN** automation prepares or updates a release PR
- **THEN** package version, lockfile version, and man-page version agree before the PR is eligible for release

#### Scenario: Header is malformed
- **WHEN** metadata preparation finds zero or multiple matching headers
- **THEN** preparation fails visibly and no release tag is created

### Requirement: Checked post-merge tagging
The system SHALL create `vX.Y.Z` only for a merged release PR after formatting, Clippy, tests, and metadata checks succeed on the exact commit being tagged. Other pushes and unmerged release branches MUST NOT create release tags.

#### Scenario: Release PR merges successfully
- **WHEN** a release PR merges into `main` and its resulting commit passes required checks
- **THEN** automation tags that commit with its package version

#### Scenario: Checks fail or an ordinary PR merges
- **WHEN** required checks fail or the merged change is not a release PR
- **THEN** automation creates no release tag

### Requirement: Safe tag reruns
The system MUST NOT move or overwrite an existing release tag.

#### Scenario: Existing tag is encountered
- **WHEN** a release run finds the expected tag already present
- **THEN** it accepts the tag only if it identifies the intended commit and otherwise fails visibly

### Requirement: Existing distribution handoff
Automation SHALL trigger the existing tag-based distribution pipeline after successful tagging. That pipeline SHALL remain the sole automated publisher for GitHub Release artifacts, crates.io, npm, and Homebrew.

#### Scenario: Automated tag is pushed
- **WHEN** automation pushes a new release tag
- **THEN** the existing distribution workflow starts without requiring a manual tag push or duplicate publisher

### Requirement: Single version-management tool
The documented normal release process SHALL use one version-management tool and SHALL preserve an explicit local distribution fallback without requiring cargo-release.

#### Scenario: Maintainer follows release documentation
- **WHEN** a maintainer prepares a normal release
- **THEN** documentation directs them to review and merge the automated release PR and explains required GitHub setup and failed-publish recovery

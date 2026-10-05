# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/hankcraft/git-ca/compare/v0.2.6...v0.3.0) - 2026-10-05

### Added

- *(release)* gate immutable merge tags on CI and release PR identity
- *(release)* prepare versions and synchronize manual metadata
- *(rules)* select drafting rules from local Git config
- *(cli)* add invocation-specific writing rules files
- *(dev)* install temporary checkout versions with commit metadata

### Fixed

- *(release)* amend metadata into the release commit
- *(release)* name pinned checkout because release-pr rejects detached HEAD
- *(pr)* push current branch and disable implicit gh forking

### Other

- *(release)* record verified merge tagging and immutable reruns
- *(release)* record verified App preparation and update behavior
- *(release)* record verified implementation phases
- *(release)* migrate maintainers to reviewed release automation
- *(release)* tag checked merge commits explicitly
- *(release)* propose release-plz automation
- *(rules)* explain repository-local rules selection
- *(openspec)* propose automatic local rules file selection
- *(openspec)* plan rules file implementation and verification
- *(openspec)* define rules file selection contract
- configure graft repository context tooling
- *(openspec)* initialize spec-driven workflow for Codex
- ignore graft cache while keeping cards searchable

## 1. Release configuration and metadata

- [x] 1.1 Add release-plz configuration from design.md; verify it reads existing `v*` tags, proposes expected `0.x` bumps, and disables its registry/GitHub Release publishers and automatic tags.
- [x] 1.2 Implement man-page update/check helper and one runnable check covering mismatched versions, unchanged reruns, and missing/duplicate headers; verify unrelated content is preserved and lockfile version agrees.

## 2. CI and release orchestration

- [x] 2.1 Add required PR/main checks for formatting, Clippy, tests, and metadata; verify a deliberate mismatch fails and a synchronized release PR passes.
- [x] 2.2 Add App-authenticated, serialized workflow pinned to the event SHA, with explicit merged-release-PR validation, immutable merge-SHA tagging, and post-PR metadata reconciliation; verify workflow syntax and preparation/update behavior without publishing.
- [ ] 2.3 Verify in a disposable repository with dummy publishers that ordinary PRs/failing checks create no tag, release PR merges tag the checked SHA, bot pushes trigger checks/distribution, and reruns preserve tags or fail on conflicting targets.

## 3. Migration and documentation

- [x] 3.1 Remove `release.toml` and replace cargo-release instructions in `docs/development.md`; verify no active release instructions require cargo-release and metadata behavior is covered by the helper check.
- [x] 3.2 Document App permissions/secrets, required checks, maintainer `dev`/`main` cutover, local fallback, and partial-publish recovery; verify documentation against existing workflows without changing GitHub settings or publishing a real release.
- [x] 3.3 Run helper checks, workflow validation, standard Rust checks, and `dist plan`; verify generated release/publishing files remain unchanged, then commit implementation phases atomically.

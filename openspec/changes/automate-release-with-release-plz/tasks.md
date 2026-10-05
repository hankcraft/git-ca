## 1. Release configuration and metadata

- [x] 1.1 Add release-plz configuration from design.md; verify it reads existing `v*` tags, proposes expected `0.x` bumps, and disables its registry/GitHub Release publishers and automatic tags.
- [x] 1.2 Implement man-page update/check helper and one runnable check covering mismatched versions, unchanged reruns, and missing/duplicate headers; verify unrelated content is preserved and lockfile version agrees.

## 2. CI and release orchestration

- [x] 2.1 Add required PR/main checks for formatting, Clippy, tests, and metadata; verify a deliberate mismatch fails and a synchronized release PR passes.
- [x] 2.2 Add App-authenticated, serialized workflow pinned to the event SHA, with explicit merged-release-PR validation, immutable merge-SHA tagging, and post-PR metadata reconciliation; verify workflow syntax and preparation/update behavior without publishing.
- [x] 2.3 Verify in a disposable repository with dummy publishers that ordinary PRs/failing checks create no tag, release PR merges tag the checked SHA, bot pushes trigger checks/distribution, and reruns preserve tags or fail on conflicting targets.

## 3. Migration and documentation

- [x] 3.1 Remove `release.toml` and replace cargo-release instructions in `docs/development.md`; verify no active release instructions require cargo-release and metadata behavior is covered by the helper check.
- [x] 3.2 Document App permissions/secrets, required checks, maintainer `dev`/`main` cutover, local fallback, and partial-publish recovery; verify documentation against existing workflows without changing GitHub settings or publishing a real release.
- [x] 3.3 Run helper checks, workflow validation, standard Rust checks, and `dist plan`; verify generated release/publishing files remain unchanged, then commit implementation phases atomically.

## Verification evidence

- Disposable `hankcraft/tmp` uses the implementation workflows and metadata/tag helpers with a tiny Rust fixture and a dummy tag publisher; no production publishing credentials or workflows were copied.
- [Mismatched metadata](https://github.com/hankcraft/tmp/actions/runs/37331367997) failed at the metadata check and skipped release; [synchronized ordinary PR](https://github.com/hankcraft/tmp/actions/runs/37331570454) passed without tagging.
- [Preparation](https://github.com/hankcraft/tmp/actions/runs/37334762492) and [update](https://github.com/hankcraft/tmp/actions/runs/37335208634) maintained the same [App-authored release PR #3](https://github.com/hankcraft/tmp/pull/3). Its final head passed bot-triggered checks with manifest, lockfile, changelog, and manual updates.
- [Release merge CI and rerun](https://github.com/hankcraft/tmp/actions/runs/37335459991) tagged merge SHA `0a6ecd12ffc73052fd140830e8ad9c9f0d573eac`, not PR head `a9d8750c4fdd64036f61cbca28feb6dd059a2d4f`; the rerun accepted the existing tag unchanged. [App-triggered dummy distribution](https://github.com/hankcraft/tmp/actions/runs/37335564199) succeeded.
- [Conflicting-tag test](https://github.com/hankcraft/tmp/actions/runs/37336199992) passed checks, then intentionally failed at tagging: `v0.3.1` already pointed to baseline `42ed1275468cc7407cd50a6e4a2ecf39995ac6a6`. The target remained unchanged. This deliberate failure and the disposable test fixtures are retained for inspection.
- Local helper checks, formatting, Clippy, all 130 Rust tests (none skipped), `dist plan`, and OpenSpec validation passed. Actionlint passed with only its outdated `concurrency.queue` schema check excluded; GitHub accepted and executed that syntax. Generated distribution and channel publishing files remain unchanged.

## 1. Settings and CLI

- [x] 1.1 Add typed inference settings, global flags, optional config fields, set/get/reset commands, and config-list output; verify CLI placement, invalid values, legacy config loading, round trips, and file permissions with focused tests.

## 2. Generation and transport

- [x] 2.1 Thread settings through commit/PR orchestration and resolve per-field precedence in shared generation; verify explicit false overrides saved true, flags do not persist, Copilot rejects explicit flags but ignores saved values, and non-drafting commands ignore flags.
- [x] 2.2 Add optional Codex request fields and update callers/exports; verify wiremock bodies for omission, priority/default tiers, all four reasoning levels, combined settings, and retry preservation. Retain existing stream and error tests.
- [x] 2.3 Verify commit and PR create/update paths with both source modes use resolved settings without changing editor or output contracts; use focused orchestration/integration tests and preserve explicit hidden-model behavior.

## 3. Documentation and completion

- [x] 3.1 Update README, development notes, Codex API guidelines, and man page; verify examples match CLI help and cover precedence, reset, provider scope, increased usage, and model-dependent availability.
- [x] 3.2 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and strict OpenSpec validation; record results and any skips. Commit implementation in focused Conventional Commits without pushing.

## Verification

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test`: 137 tests passed (134 unit, 3 integration); none ignored.
- `openspec validate add-codex-inference-controls --strict`: passed.
- CLI help matches documented flags, effort choices and config commands.
- `groff -z -man -Tutf8 docs/man/git-ca.1`: passed without diagnostics.
- `graft build`: refreshed local context graph.
- No paid live generation or cross-platform runs; orchestration tests run on Unix with local HTTP mocks and fake Git/gh/editor commands. Backend model/account availability remains unverified live.
- Implementation and documentation committed separately with Conventional Commits; no push.

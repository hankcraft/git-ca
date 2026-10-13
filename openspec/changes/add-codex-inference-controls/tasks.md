## 1. Settings and CLI

- [x] 1.1 Add typed inference settings, global flags, optional config fields, set/get/reset commands, and config-list output; verify CLI placement, invalid values, legacy config loading, round trips, and file permissions with focused tests.

## 2. Generation and transport

- [ ] 2.1 Thread settings through commit/PR orchestration and resolve per-field precedence in shared generation; verify explicit false overrides saved true, flags do not persist, Copilot rejects explicit flags but ignores saved values, and non-drafting commands ignore flags.
- [ ] 2.2 Add optional Codex request fields and update callers/exports; verify wiremock bodies for omission, priority/default tiers, all four reasoning levels, combined settings, and retry preservation. Retain existing stream and error tests.
- [ ] 2.3 Verify commit and PR create/update paths with both source modes use resolved settings without changing editor or output contracts; use focused orchestration/integration tests and preserve explicit hidden-model behavior.

## 3. Documentation and completion

- [ ] 3.1 Update README, development notes, Codex API guidelines, and man page; verify examples match CLI help and cover precedence, reset, provider scope, increased usage, and model-dependent availability.
- [ ] 3.2 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and strict OpenSpec validation; record results and any skips. Commit implementation in focused Conventional Commits without pushing.

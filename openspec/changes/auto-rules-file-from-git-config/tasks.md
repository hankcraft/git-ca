## 1. Local Git lookup and shared resolution

- [x] 1.1 Add focused optional `ca.rulesFile` lookup in `src/git/mod.rs`, distinguishing absent keys from errors and preserving path spaces; verify with isolated temporary Git repositories covering absent, local, global-only, duplicate, empty, and failed lookups.
- [x] 1.2 Extend shared rules resolution in `src/main.rs` with flag-first selection and repository-root joining for relative configured paths; verify resolver tests covering flag bypass, absolute and subdirectory paths, unchanged defaults, strict configured-file validation, and preserved prompt contracts.

## 2. Drafting integration

- [x] 2.1 Extend `tests/rules_file.rs` and its fake Git command handling for the new read-only queries; verify commit and both PR source flows, configured-file errors before generation/editor/mutations, flag overrides, unchanged missing-key fallback, and no lookup for auth/models/config using `cargo test --test rules_file`.
- [x] 2.2 Verify configuration isolation and repository-root behavior from actual repository subdirectories, including linked worktrees; add focused real-Git checks where the fake-command fixture cannot prove these behaviors and run those tests.

## 3. Documentation and final verification

- [x] 3.1 Update `README.md`, `docs/development.md`, and `docs/man/git-ca.1` with local setup/unset commands, precedence, distinct path bases, and strict failures; review examples against the spec and remove claims that defaults always apply whenever the flag is absent.
- [x] 3.2 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`; verify all checks pass and report any skipped checks. Commit implementation and documentation in atomic Conventional Commits without pushing.

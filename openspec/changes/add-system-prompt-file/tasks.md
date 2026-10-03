## 1. CLI and file resolution

- [ ] 1.1 Add global optional `--rules-file <PATH>` argument backed by `PathBuf` in `src/cli.rs` and thread it through `run` to both drafting commands; verify parsing with no flag yielding `None`, commit invocation, before/after `pr`, both PR sources, and existing global options.
- [ ] 1.2 Add one shared resolver in `src/main.rs` with strict explicit reads and existing optional default loader; verify explicit precedence, verbatim UTF-8 contents, relative/absolute paths, literal special paths, and path-bearing exit-1 errors for missing, directory, invalid UTF-8, and whitespace-only files without fallback.

## 2. Drafting integration and compatibility

- [ ] 2.1 Connect both drafting commands to the resolver before generation; verify each no-flag command selects its own default file and preserves missing/invalid default behavior, using existing tests and focused additions.
- [ ] 2.2 Add resolution-to-prompt checks for commit and both PR sources; verify fixed prefixes remain, default rules disappear, and source stays separate. Verify invalid explicit input blocks generation/editor/Git/gh paths and unrelated commands ignore invalid paths. Preserve existing provider, response validation, auto-accept, and hook checks.

## 3. Documentation and final verification

- [ ] 3.1 Update README, both runtime flows in `docs/development.md`, and `docs/man/git-ca.1`; verify examples, precedence, literal path semantics, strict errors, and rules-only wording agree with specs and both CLI help screens.
- [ ] 3.2 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`; confirm all pass, review focused diff, and commit implementation atomically without pushing. Validation requires no live commit or PR creation/update.

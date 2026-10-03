## Why

Commit and PR drafting currently accept custom rules only through fixed files in
the user's config directory. An explicit file argument lets each invocation use
project or workflow rules without changing shared configuration.

## What Changes

- Add global `--rules-file <PATH>` for `git ca` and `git ca pr`.
- Replace only writing rules, retaining fixed Conventional Commits instructions
  for commits and the JSON response contract for PRs.
- Prefer the explicit file; reject invalid explicit inputs before generation.
- Preserve command-specific default files and their existing fallback behavior.
- Document path semantics, precedence, errors, and examples for both commands.

## Capabilities

### New Capabilities

- `system-prompt-file`: Invocation-specific rule files for commit and PR drafts,
  including selection, validation, fallback, and preserved output contracts.

### Modified Capabilities

None. No existing OpenSpec capabilities are registered.

## Impact

CLI definition and command dispatch in `src/cli.rs` and `src/main.rs`; nearby
tests; README, runtime notes, and man page. Existing prompt builders, provider
transport, config paths, editor review, and Git/gh execution remain compatible.
No new dependencies, persistent keys, or breaking changes. Technical approach
and usage examples are maintained in this change's `design.md`.

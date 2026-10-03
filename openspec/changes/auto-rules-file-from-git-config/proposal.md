## Why

Repositories cannot currently select shared drafting rules without passing `--rules-file` on every invocation. Reading repository-local `ca.rulesFile` makes that selection automatic for commit and PR drafting.

## What Changes

- Read `ca.rulesFile` from repository-local Git config when `--rules-file` is absent.
- Use precedence: explicit flag, local Git setting, command-specific default file, built-in rules.
- Resolve relative configured paths from the repository root; retain working-directory resolution for explicit flag paths.
- Validate configured files strictly, using the existing rules-file validation and prompt contracts.
- Document setup with `git config --local ca.rulesFile prompts/rules.md`, overrides, and removal.

## Capabilities

### New Capabilities

- `git-config-rules-file`: Automatic repository-local rules selection, precedence, path resolution, and validation for commit and PR drafting.

### Modified Capabilities

None. No main capabilities are published under `openspec/specs/` yet. The completed `add-system-prompt-file` change supplies the existing behavior; this proposal extends its default-selection condition to require both the flag and local setting to be absent.

## Impact

Rules resolution in `src/main.rs`, Git queries in `src/git/mod.rs`, and existing tests in `tests/rules_file.rs`. Documentation changes belong in `README.md`, `docs/development.md`, and `docs/man/git-ca.1`. No new dependencies, configuration JSON keys, or authentication changes. Global/system Git settings remain outside this local-only scope.

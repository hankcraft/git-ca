## Purpose

Select repository-specific writing rules automatically from local Git configuration for commit and PR drafts, with predictable overrides and paths.

## ADDED Requirements

### Requirement: Repository-local automatic rules selection

Without `--rules-file`, commit and PR drafting SHALL read repository-local `ca.rulesFile` using Git's local configuration lookup. A configured path SHALL select the same file for commits and PR creation or updates with either source. Global and system settings SHALL NOT supply this selection. If multiple local values exist, the last value SHALL win, matching Git's single-value lookup.

#### Scenario: Automatic drafting rules
- **WHEN** `git config --local ca.rulesFile prompts/rules.md` is set and drafting runs without the flag
- **THEN** commit drafting and PR drafting with either source, for creation or updates, use that file's rules

#### Scenario: Global setting only
- **WHEN** the key exists only in global or system Git config
- **THEN** drafting uses existing command-specific default resolution

### Requirement: Explicit override and compatible defaults

`--rules-file` SHALL take precedence and bypass local rules-config lookup. When both the flag and local key are absent, existing command-specific default files and built-in fallback behavior SHALL remain unchanged. Selecting either the flag or local key SHALL bypass default-file reads and SHALL NOT persist any settings.

#### Scenario: Explicit override of invalid local rules
- **WHEN** a valid flag path is supplied alongside an invalid configured rules path
- **THEN** only the flag file is read and no configured-file error occurs

#### Scenario: Key unset
- **WHEN** the flag is absent and the local key is unset or removed
- **THEN** commit and PR drafting use their own default files with existing warning and built-in fallback behavior

### Requirement: Configured path semantics

Relative configured paths SHALL resolve from the repository's work-tree root, including when drafting runs in a subdirectory. Absolute configured paths SHALL be used directly. Spaces in paths SHALL be preserved. Flag paths SHALL retain invocation-working-directory resolution. Files SHALL be read once as UTF-8 with contents preserved verbatim, without extension restrictions, tilde expansion, interpolation, or special stdin handling.

#### Scenario: Invocation in subdirectory
- **WHEN** local config names `prompts/team rules.md` and drafting runs from a repository subdirectory
- **THEN** the selected file is `prompts/team rules.md` under the repository root

#### Scenario: Absolute configured path
- **WHEN** local config names an absolute path
- **THEN** that exact file is selected regardless of invocation directory

#### Scenario: Relative explicit override
- **WHEN** drafting in a subdirectory passes `--rules-file ./rules.md`
- **THEN** the file is resolved from that subdirectory

### Requirement: Strict configured-file validation

A present empty or whitespace-only setting, or a configured missing, unreadable, directory, non-UTF-8, empty, or whitespace-only file SHALL cause exit 1 with a diagnostic identifying `ca.rulesFile` and the path and reason where applicable, without printing file contents. Git configuration lookup failures other than an absent key SHALL stop drafting with a diagnostic. These failures SHALL NOT fall back, generate text, invoke an editor, commit, or create/update a PR. Existing repository, configuration, and source preconditions MAY fail first.

#### Scenario: Invalid configured file with valid default
- **WHEN** a configured file has any listed validation failure even though a valid default exists
- **THEN** drafting fails before generation or mutation and identifies the setting and failing path

#### Scenario: Empty configured value
- **WHEN** the local key exists with an empty or whitespace-only value
- **THEN** drafting reports invalid `ca.rulesFile` and exits 1 rather than treating it as absent

#### Scenario: Git config lookup failure
- **WHEN** reading local config fails for a reason other than an absent key
- **THEN** drafting reports the failure and does not generate text or mutate Git or PR state

### Requirement: Drafting-only lookup and preserved contracts

Auth, models, and config commands SHALL ignore `ca.rulesFile` without looking it up or validating it. Configured custom rules SHALL replace only the Rules section and retain existing commit and PR output contracts, provider selection, editor review, auto-accept, and commit-hook behavior.

#### Scenario: Non-drafting command
- **WHEN** auth, models, or config runs with an invalid configured rules path
- **THEN** no rules-config lookup or rules-file validation occurs

#### Scenario: Prompt contracts
- **WHEN** configured custom rules are selected for either drafting command
- **THEN** the existing fixed instructions and separate source input remain, and default rules are replaced

## Purpose

Allow commit and PR drafts to use invocation-specific writing rules from a file
while preserving existing output contracts and default-file compatibility.

## ADDED Requirements

### Requirement: Global rules file argument

The CLI SHALL accept optional `--rules-file <PATH>` for commit and PR
drafting, including before or after `pr`, with existing global options and either
PR source. PR drafting SHALL apply the selection to both creation and updates.
It SHALL have no short alias. Non-drafting commands SHALL accept but
ignore it without reading or validating the supplied file.

#### Scenario: Commit drafting
- **WHEN** `git ca --rules-file ./commit.md` drafts a commit
- **THEN** it selects `./commit.md` for custom rules

#### Scenario: PR drafting
- **WHEN** the flag is supplied before or after `pr`, using diff or commits source for creation or update
- **THEN** PR drafting selects the supplied path

#### Scenario: Unrelated command
- **WHEN** auth, models, or config runs with an invalid rules-file path
- **THEN** the path is ignored and does not cause a rules-file error

### Requirement: Explicit file precedence

An explicit file SHALL replace the invoked command's default rule file for that
invocation only. The default file SHALL NOT be read and no setting SHALL be persisted.

#### Scenario: Conflicting files
- **WHEN** explicit and default files contain different valid rules
- **THEN** only explicit rules are used and shared configuration is unchanged

### Requirement: File and path semantics

The selected file SHALL be read once as UTF-8. Relative paths SHALL resolve from
the invocation's working directory, including repository subdirectories;
absolute paths SHALL work directly. Non-empty contents SHALL be preserved
verbatim, including whitespace. No extension restriction, interpolation, custom
tilde expansion, or stdin syntax SHALL apply; `-` SHALL mean a literal filename.

#### Scenario: Valid file contents
- **WHEN** a relative or absolute path identifies non-empty UTF-8 rules
- **THEN** the rules are loaded with original whitespace and trailing newlines

#### Scenario: Literal special path
- **WHEN** PATH is `-`, a quoted tilde path, or contains interpolation-like text
- **THEN** it is handled as a filesystem path without stdin or template processing

### Requirement: Strict explicit file validation

An explicit missing, unreadable, directory, non-UTF-8, or whitespace-only file
SHALL cause exit 1 and a diagnostic naming the supplied path and reason without
printing contents. It SHALL NOT fall back, request model generation, invoke an
editor, commit, or create/update a PR. Existing repository, configuration, and
source preconditions MAY fail first.

#### Scenario: Invalid explicit file
- **WHEN** any listed explicit-file validation failure occurs, even with a valid default file
- **THEN** drafting fails with the specified diagnostic and no generation or mutation

### Requirement: Compatible default resolution

Without the flag, commit drafting SHALL use `commit-system-prompt.md` and PR
drafting SHALL use `pr-system-prompt.md` under `$XDG_CONFIG_HOME/git-ca`, or
`~/.config/git-ca` when XDG is unset or empty. Each command SHALL read only its
own default file. Missing files SHALL silently select built-in rules; empty or
unreadable files SHALL warn and select built-in rules.

#### Scenario: Valid command-specific default
- **WHEN** no flag is supplied and both default files contain different valid rules
- **THEN** each drafting command uses its own default rules

#### Scenario: Missing default
- **WHEN** no flag is supplied and the command's default file is missing
- **THEN** built-in rules are used without a prompt-file warning

#### Scenario: Invalid default
- **WHEN** no flag is supplied and the command's default file is empty or unreadable
- **THEN** a warning is printed and built-in rules are used

### Requirement: Preserve prompt and output contracts

Custom contents SHALL replace only the Rules section, not append to default
rules. Fixed Conventional Commits instructions SHALL remain for commits; fixed
PR role and JSON instructions SHALL remain for PRs. Source input SHALL remain
separate from system rules. Existing response cleanup, PR validation, provider
selection, editor review, auto-accept, and commit-hook behavior SHALL remain unchanged.
Commit output SHALL remain plain Conventional Commits text with existing response cleanup.

#### Scenario: Custom commit rules
- **WHEN** commit drafting uses custom rules with either provider
- **THEN** system instructions retain the Conventional Commits prefix and exclude default rules

#### Scenario: Custom PR rules
- **WHEN** PR drafting uses custom rules with either provider and either source
- **THEN** system instructions retain the JSON prefix, exclude default rules, and keep source separate
- **AND** generated PR output is still validated as non-empty title and body strings

## Purpose

Let users choose Codex service speed and reasoning effort for commit and pull request drafts while preserving existing defaults and provider behavior.

## ADDED Requirements

### Requirement: Invocation controls
The CLI SHALL accept global `--fast`, `--fast=true`, `--fast=false`, and `--reasoning <low|medium|high|xhigh>` before or after drafting subcommands. Both controls SHALL apply to commits and PR creation or updates with either diff or commits source. Invalid values MUST fail before generation or Git/gh mutations.

#### Scenario: Combined controls on PR drafting
- **WHEN** a Codex user runs PR drafting with `--fast --reasoning low`
- **THEN** both settings apply to the generation request

#### Scenario: Invalid reasoning
- **WHEN** the user supplies `--reasoning bogus`
- **THEN** the command fails without generating text or committing/updating a PR

### Requirement: Persistent defaults and precedence
Configuration SHALL support optional `codex_fast` boolean and `codex_reasoning` effort keys. `config set-codex-fast <true|false>`, `get-codex-fast`, `set-codex-reasoning <low|medium|high|xhigh|default>`, `get-codex-reasoning`, and `list` SHALL expose them. Setting reasoning to `default` SHALL remove its override. Missing values SHALL display as `default (backend)`. Each flag SHALL override only its corresponding saved value without persisting the flag. Existing files and restrictive permissions MUST remain valid.

#### Scenario: Disable saved fast mode for one invocation
- **WHEN** `codex_fast` is true and drafting supplies `--fast=false`
- **THEN** that invocation uses Standard processing and saved configuration remains true

#### Scenario: Restore model reasoning default
- **WHEN** the user sets Codex reasoning to `default`
- **THEN** the saved reasoning override is removed and subsequent drafts omit it

### Requirement: Codex request semantics
Enabled fast mode SHALL serialize as `service_tier: "priority"`; explicit disabled mode SHALL serialize as `service_tier: "default"`. Selected reasoning SHALL serialize as `reasoning: {"effort": "<value>"}`. Unset controls MUST omit their respective fields. These controls SHALL remain independent, including during authentication retries. Backend rejection MUST surface through existing error handling without silently changing settings.

#### Scenario: Existing defaults preserved
- **WHEN** neither flags nor saved inference settings exist
- **THEN** the Codex request contains neither `service_tier` nor `reasoning`

#### Scenario: Reasoning selection without fast mode
- **WHEN** only `--reasoning high` is selected
- **THEN** the request contains reasoning effort high and omits service tier

### Requirement: Provider scope and compatibility
Copilot drafting MUST reject explicit inference flags before generation, but SHALL ignore saved Codex settings. Auth, models, and config commands SHALL ignore valid global inference flags. Existing model selection, hidden explicit Codex slugs, prompt contracts, editor review, and Git/gh behavior SHALL remain unchanged.

#### Scenario: Switch to Copilot with saved Codex defaults
- **WHEN** the active provider is Copilot and only saved Codex inference settings exist
- **THEN** drafting proceeds with the existing Copilot request format

#### Scenario: Explicit Codex flag on Copilot
- **WHEN** Copilot drafting receives either inference flag
- **THEN** the command fails with a Codex-only diagnostic before generation

### Requirement: Usage guidance
User documentation and flag help SHALL identify fast mode's increased usage and model/account restrictions, distinguish speed tier from reasoning depth, and describe precedence and supported effort choices.

#### Scenario: Discover fast mode costs
- **WHEN** a user reads drafting help or documented fast mode examples
- **THEN** increased usage is disclosed before they enable the option

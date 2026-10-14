## Why

git-ca constructs Codex requests independently of Codex CLI settings and currently exposes neither fast mode nor reasoning effort. Users need explicit control over drafting latency and reasoning depth for commit and PR text without changing existing defaults.

## What Changes

- Add global `--fast[=true|false]` and `--reasoning <low|medium|high|xhigh>` for commit and PR drafting.
- Add optional `codex_fast` and `codex_reasoning` configuration keys with set/get commands following existing config conventions.
- Resolve flags before saved settings; omit unset Codex request fields. Map enabled fast mode to `service_tier: "priority"` and reasoning to `reasoning.effort`.
- Reject explicit inference flags for Copilot drafting; ignore saved Codex settings when using Copilot. Non-drafting commands ignore global flags.
- Document usage costs, supported choices, precedence, and the independence of these controls.

## Capabilities

### New Capabilities

- `codex-inference-controls`: Optional fast mode and reasoning selection across commit and PR drafting, persistence, provider scope, and request serialization.

### Modified Capabilities

None. `openspec list --specs` reports no registered capabilities.

## Impact

`src/cli.rs`, `src/main.rs`, `src/config/mod.rs`, Codex request types and exports in `src/codex/`, existing CLI/config/wiremock tests, README, development notes, Codex API guidelines, and `docs/man/git-ca.1`. No dependency additions or changes to credentials, model selection, prompt contracts, editor defaults, or Git/gh execution. Existing configuration files remain valid.

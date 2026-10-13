## Context

See proposal.md for motivation and the delta spec for behavior. `Cli` and `ConfigAction` live in `src/cli.rs`; `Config` persists JSON through existing permission-preserving helpers. `run` dispatches to commit and PR functions, which share `generate_text` in `src/main.rs:185`. The Codex branch calls `Client::chat`; `ResponsesRequest` in `src/codex/chat.rs:17` currently lacks inference controls. Its wiremock tests exercise streamed text and error mapping.

The Codex catalog deserializer retains only model identity, visibility, and ordering. Existing model setters allow explicit Codex slugs outside the visible catalog. No registered main specs conflict with this addition.

## Goals / Non-Goals

**Goals:** Resolve settings once through existing orchestration, preserve retry behavior, and keep absent settings distinct from explicit false/default values.

**Non-Goals:** Interactive pickers, catalog display changes, per-model/account settings, new inference controls for Copilot, and reading Codex CLI config. These would expand scope beyond choosing settings for drafting.

## Decisions

1. Use optional typed values for CLI/config settings. A shared reasoning enum covers `low`, `medium`, `high`, and `xhigh`, the four levels advertised for the current `gpt-5.5` fallback. Clap and serde reject invalid choices. `--fast` uses an optional boolean value requiring `=` when supplied; absence stays distinct from false. Config commands mirror existing set/get patterns; reasoning `default` clears its optional value. Prefer these explicit settings over importing unrelated Codex CLI configuration.

2. Resolve each value as CLI override, then saved setting, then unset in the shared generation path. Carry resolved options through the Codex auth closure into `Client::chat`, reusing them on retries. Use the smallest options value needed by these callers and expose it through existing Codex exports. Reject explicit flags for Copilot in this path; do not reject persisted Codex settings when switching providers.

3. Serialize optional service tier and optional reasoning object with omission for absent values. True maps to `priority`, false to `default`. Omission preserves existing backend defaults; explicit false forces Standard processing. Reasoning remains independent of fast mode.

4. Do not fetch the model catalog during drafting or gate explicit slugs on visibility. The backend remains the availability check, as with existing model selection. Dynamic catalog-driven validation would add a request and could break accepted hidden slugs; local validation checks spelling only.

5. Preserve unset values when loading old config files and use current JSON writing helpers for set commands. Add config-list formatting cases, argument parsing tests, precedence/provider tests, and request-body matchers to existing wiremock coverage. Test omission as well as explicit fields so backward compatibility is enforced.

## Risks / Trade-offs

- Fast mode consumes more usage and requires model/account support → opt-in only; explain costs and expose backend errors without fallback.
- Models can support different reasoning levels → document the initial four-level interface and backend validation; catalog-driven expansion is separate work.
- ChatGPT backend contract is not a stable public API → use `priority`, advertised by the local catalog, and retain existing error mapping. No paid live generation is required for automated verification.

## Migration Plan

Add optional config fields without rewriting existing files. Ship code, tests, and documentation in focused commits. Rollback by reverting the feature; older serde readers ignore added config fields. No authentication or credential migration is needed.

## Evidence

Study verified installed Codex `0.162.1` and its cached `gpt-5.5` metadata: default reasoning `medium`, levels `low/medium/high/xhigh`, service tier `priority` named Fast. No live generation was tested. Official references: [configuration](https://learn.chatgpt.com/docs/config-file/config-reference), [speed and usage](https://learn.chatgpt.com/docs/agent-configuration/speed), and [reasoning](https://developers.openai.com/api/docs/guides/reasoning).

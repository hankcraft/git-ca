## Context

See proposal.md for motivation. Both drafting commands call `resolve_system_prompt_file` in `src/main.rs:123-140`. Explicit files already receive strict UTF-8/non-empty validation; default files use permissive fallback. `src/git/mod.rs:11-27` runs Git queries, but its generic helper treats every nonzero exit as an error. `tests/rules_file.rs:6-131` uses fake Git/gh commands to verify resolution and prevent generation or mutation on invalid input.

The completed `add-system-prompt-file` delta describes defaults whenever the flag is absent; this change intentionally narrows that condition to absence of both flag and local key. Preserve its flag path semantics and all other contracts. No main spec exists yet; reconcile that condition when the changes are synced or archived.

## Goals / Non-Goals

**Goals:** Keep selection shared between commit and PR flows, preserve literal paths, and reuse strict file validation.

**Non-Goals:** Global/system configuration, separate commit/PR keys, worktree-specific overrides, JSON settings, or new dependencies.

## Decisions

1. Query `git config --local --get ca.rulesFile` only when the flag is absent, inside the drafting resolver. Use a small Git helper that returns an optional path: status 1 with no diagnostic means an absent key; other failures propagate with a useful diagnostic. The existing capture helper cannot represent this normal missing-key outcome, so do not change its behavior for unrelated callers. Use Git rather than parsing `.git/config`, preserving repository discovery and linked-worktree support.
2. For a relative configured path, get the current work-tree root using `git rev-parse --show-toplevel` and join the path. Absolute configured paths and explicit flag paths need no root query. Repository-root resolution follows the user's choice and keeps config stable across invocation directories. Remove only command-output terminators, preserving path spaces; reject empty/whitespace-only configured values.
3. Route selected paths through existing strict file loading. Annotate configured-file failures with `ca.rulesFile` and the resolved path. Keep default fallback only for an absent setting. This avoids silently discarding repository policy. Do not read defaults or query the key when the explicit flag wins.
4. Extend existing tests rather than adding infrastructure. Update fake Git to distinguish root queries and config lookup, and retain assertions excluding provider generation, editors, commits, and PR mutations. Use temporary real Git repositories for local/global scope and root-relative behavior; isolate Git configuration in those tests.

## Risks / Trade-offs

- Additional Git queries when the flag is absent → one key lookup, with root lookup only for configured relative paths.
- Generic Git helper reports missing keys as errors → handle absence in the focused lookup helper, keeping existing callers unchanged.
- Existing fake Git rejects new commands → teach the fixture those read-only queries and preserve side-effect assertions.
- Rules referenced by local config can be deleted → fail with a path-bearing diagnostic; document unsetting the key to restore defaults.

## Migration Plan

No automatic migration or config writes. Existing repositories without the key keep current behavior. Opt in with `git config --local ca.rulesFile prompts/rules.md`; remove with `git config --local --unset-all ca.rulesFile`. A per-invocation flag overrides the setting. Rollback requires removing the key or using the prior binary.

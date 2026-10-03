## Context

See proposal.md for motivation and `specs/system-prompt-file/spec.md` for the
contract. Status: design only; implementation pending.

`src/main.rs` currently orchestrates both drafting commands and shares optional
`load_system_prompt_file`. `src/config/paths.rs` selects their separate default
files. Both prompt builders already replace rules while retaining fixed prefixes;
their tests verify this. `src/cli.rs` declares drafting options as global fields.

## Goals / Non-Goals

**Goals:** Keep one selection/validation policy for both callers; preserve
existing prompt builders, default-file behavior, and execution paths.

**Non-Goals:** Full prompt replacement, persistent paths, automatic repository
discovery, templates, stdin support, dependencies, or transport changes.

## Decisions

1. Add global `rules_file: Option<PathBuf>` to `Cli` as `--rules-file` with no
   short alias. The name reflects rules-only replacement. Thread it through
   `run` into `commit` and `pull_request`. Separate root/PR flags
   would duplicate parsing and diverge from `--model`/`--yes`; global acceptance
   also means unrelated commands ignore the option without touching its file.
2. Share one small resolver in `src/main.rs`. An explicit path uses
   `std::fs::read_to_string`, rejects `content.trim().is_empty()`, and returns
   path-bearing `Error::Config` errors. The no-flag branch calls existing optional
   loader with the command's default path and label. Keep that loader and its
   existing tests unchanged. Reusing its permissive fallback for explicit input
   would silently substitute unexpected rules; duplicating strict logic would
   allow commit and PR behavior to drift.
3. Pass resolved contents into `commit_msg::prompt::build` or
   `pr_msg::prompt::build` before `generate_text`.
   Preserve valid contents verbatim. Full system replacement would remove output
   instructions required by downstream consumers. Copilot already carries the
   system message; Codex already maps it into `instructions`.
4. Document the argument in README, both runtime flows, and man page. Correct
   README's full-replacement wording: implementation, tests, and man page agree
   on rules-only replacement. No existing OpenSpec capability conflicts exist.

## Risks / Trade-offs

- Custom rules omit default writing safeguards → Document replacement semantics
  and include examples retaining evidence-based writing rules.
- Conflicting custom rules can still produce invalid responses → Retain fixed
  prefixes, existing cleanup, and PR JSON validation; no speculative retries.
- Invalid default and explicit files have different outcomes → Keep policy
  distinction in one resolver and test both paths.
- Permission-bit tests fail under root → Use portable directory/invalid-UTF-8
  read failures rather than assuming chmod prevents reads.

## Usage examples

```sh
git ca --rules-file ./prompts/commit.md
git ca pr --rules-file ./prompts/pr.md
git ca --rules-file ./prompts/pr.md pr
git ca pr --base develop --source commits --rules-file ./prompts/pr.md -y
```

Example PR file:

```markdown
- Write a concise imperative title.
- Use Markdown sections: Summary, Changes, Testing.
- Describe reviewer-relevant risks.
- Do not invent test results or issue references.
```

Example commit file:

```markdown
- Write the subject in imperative mood, without a trailing period.
- Include a body explaining why the change matters when supported by the diff.
- Do not invent test results or issue references.
```

Custom rules replace defaults rather than supplementing them. Users must include
any default writing requirements they want to retain. Fixed output instructions
remain: plain Conventional Commits text for commits, JSON with non-empty `title`
and `body` strings for PRs.

Resolution summary (normative contract in `specs/system-prompt-file/spec.md`):

| Input | Result |
| --- | --- |
| Explicit valid file | Use its rules; do not load the default prompt file |
| Explicit invalid file | Exit 1 with a diagnostic; do not fall back |
| No flag, valid default file | Use default file rules |
| No flag, missing default file | Use built-in rules silently |
| No flag, invalid default file | Warn and use built-in rules |

## Migration Plan

Ship an additive argument and docs; no config migration. No-flag behavior stays
compatible. Validate formatting, lint, tests, and both help screens before an
atomic implementation commit. Rollback removes the argument and resolver;
existing configuration files remain usable without conversion.

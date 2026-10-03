# git-ca

`git-ca` is a Git subcommand that drafts commit messages and pull request text using either GitHub Copilot or the OpenAI Codex (ChatGPT) backend. It reads `git diff --cached` for commits, or branch changes for PRs, asks the configured backend for a draft, opens the result in your editor by default, and then lets Git or GitHub CLI finish the action.

## Installation

**Option 1 (recommended): Install from crates.io or mainstream package managers**

```sh
cargo install git-ca
```
```sh
brew install hankcraft/tap/git-ca
```
```sh
bun install -g @hankcraft/git-ca
```
```sh
pnpm install -g @hankcraft/git-ca
```
```sh
npm install -g @hankcraft/git-ca
```


Or execute directly:
```sh
bunx @hankcraft/git-ca --help
```
```sh
pnpx @hankcraft/git-ca --help
```
```sh
npx @hankcraft/git-ca --help
```


**Option 2: Install from this checkout**

```sh
cargo install --path .
```

For a local dev version, including uncommitted source changes:

```sh
scripts/install-dev.sh
git ca --version
```

Versions look like `0.2.6-dev+abc1234` or `0.2.6-dev+abc1234.dirty`. The script
changes version metadata only in a temporary source copy, preserves locked
dependency versions, and reuses `target/` for build caching. It replaces the
Cargo-installed binary; reinstall the release when finished. Extra arguments
pass through to `cargo install`, for example `--root /tmp/git-ca-dev-install`.

To make `git ca --help` resolve through Git's man-page help path from a checkout install, also run:

```sh
install -D -m 0644 docs/man/git-ca.1 ~/.local/share/man/man1/git-ca.1
```


## Quick Start

Use `git-ca` when you have local changes and want a reviewed AI draft before committing or opening a PR.

Prerequisites:
- Git
- GitHub Copilot access or a ChatGPT account for the Codex backend
- GitHub CLI (`gh`) for PR creation

```sh
git ca auth login
git add <files>
git ca
```

For pull requests, execute under feature branch:

```sh
git ca pr
```

## Key Features

- Drafts Conventional Commits messages from staged changes.
- Drafts PR title/body text from branch diffs or commit logs and creates or updates PRs with `gh`.
- Supports GitHub Copilot and OpenAI Codex (ChatGPT) backends, with model selection.
- Opens generated text in your editor by default, with `--yes` / `-y` for direct commit or PR creation/updating.
- Supports multiple AI provider accounts, with token persistence and swapping.
- Supports custom prompts for commit and PR messages.

## Commands

| Command | Description |
| --- | --- |
| `git ca` | Draft a message for staged changes and run `git commit -e -F <message>` |
| `git ca pr` | Draft a PR title/body from current branch changes and run `gh pr create` (or `gh pr edit` if a PR already exists) |
| `git ca pr --base <branch>` | Compare the current branch against a specific PR base branch |
| `git ca pr --source commits` | Draft PR text from commit messages instead of the branch diff |
| `git ca --model <id>`, `git ca -m <id>` | Use a specific backend model for this command |
| `git ca --rules-file <PATH>`, `git ca pr --rules-file <PATH>` | Replace writing rules for this invocation with a UTF-8 file |
| `git ca --yes`, `git ca -y` | Accept generated text without opening the editor; for PRs this creates or updates the PR directly |
| `git ca --no-verify` | Pass `--no-verify` through to `git commit` |
| `git ca auth login` | Prompt for backend on a TTY, then log in (defaults to Copilot when stdin is not a TTY) |
| `git ca auth login <account>` | Same prompt behavior, then store credentials for the named account |
| `git ca auth login --provider codex [account]` | Log in via ChatGPT OAuth (PKCE) for a Codex account |
| `git ca auth set-token <token>` | Store a GitHub token manually as the default active account (Copilot only) |
| `git ca auth set-token --account <account> <token>` | Store a GitHub token manually for a named Copilot account |
| `git ca auth use <account>` | Select the named account; the active account decides the backend |
| `git ca auth logout` | Delete locally stored tokens |
| `git ca auth logout <account>` | Delete locally stored tokens for one named account |
| `git ca auth status` | Show local auth state, active account's provider, and per-provider token state |
| `git ca models` | List available models for the active account's backend |
| `git ca config list` | Print all persisted config values |
| `git ca config set-model <id>` | Persist the default model |
| `git ca config get-model` | Print the persisted default model |
| `git ca config set-auto-accept <true|false>` | Persist whether generated commit messages commit without opening the editor |
| `git ca config get-auto-accept` | Print the persisted commit auto-accept setting |
| `git ca config set-auto-accept-pr <true|false>` | Persist whether generated PRs are created without opening the editor |
| `git ca config get-auto-accept-pr` | Print the persisted PR auto-accept setting |

`auth logout` only removes local credentials. Revoke the OAuth grant separately from GitHub account settings if the server-side grant should be invalidated.

When creating a PR, `git ca pr` pushes the current branch to `origin` under the same name and passes it explicitly to `gh pr create --head`. It does not create another local branch or fork the repository. Push failures stop PR creation.

## Authentication Notes

`git ca auth login` prompts for a backend on a TTY and defaults to Copilot when stdin is piped or running in CI. Copilot supports GitHub device flow or manual token storage with `git ca auth set-token <github-token>`. Codex uses ChatGPT OAuth with a loopback callback on `127.0.0.1:1455` or fallback `:1457`.

## Copilot Request Accounting

GitHub Copilot request accounting depends on both plan and model. GitHub's
documentation is the source of truth because included models and multipliers can
change: <https://docs.github.com/en/copilot/concepts/billing/copilot-requests#model-multipliers>

## Configuration Files

`git-ca` stores configuration under `$XDG_CONFIG_HOME/git-ca` when `XDG_CONFIG_HOME` is non-empty, otherwise under `~/.config/git-ca`:

```text
~/.config/git-ca/config.json
~/.config/git-ca/auth.json
~/.config/git-ca/commit-system-prompt.md
~/.config/git-ca/pr-system-prompt.md
```

On Unix, the config directory is set to `0700` and JSON files are written with `0600` permissions.

### Available configuration keys in `config.json`:

- **default_model**: The default model to use for `git ca` and `git ca pr`.
- **auto_accept**: Whether to automatically accept generated commit messages.
- **auto_accept_pr**: Whether to automatically accept generated PR messages.

### System prompt overrides

Custom files replace only the `Rules` section. Fixed Conventional Commits instructions for commits and the PR role and JSON contract remain. Custom rules replace default writing rules rather than supplementing them; include any writing safeguards you want to keep.

Select rules for one invocation with global `--rules-file <PATH>` (no short alias), before or after `pr`. It applies to PR creation and updates with either source; auth, models, and config commands accept but ignore it.

```sh
git ca --rules-file ./prompts/commit.md
git ca pr --rules-file ./prompts/pr.md
git ca --rules-file ./prompts/pr.md pr
git ca pr --base develop --source commits --rules-file ./prompts/pr.md -y
```

For example, a PR rules file can contain:

```markdown
- Write a concise imperative title.
- Use Markdown sections: Summary, Changes, Testing.
- Describe reviewer-relevant risks.
- Do not invent test results or issue references.
```

Explicit files take precedence, bypassing local Git rules-config lookup and default-file reads without persisting a setting. Contents are read once as UTF-8 and preserved verbatim. Relative flag paths resolve from the invocation's working directory, including repository subdirectories; absolute paths work directly. Any extension is accepted. `-` means a literal filename; quoted tilde and interpolation-like text are not expanded by git-ca. There is no stdin or template syntax.

A missing, unreadable, directory, non-UTF-8, empty, or whitespace-only explicit file causes exit 1 with its path and reason, without printing contents or falling back. This stops generation, editor review, commits, and PR creation/updates. Repository, config, and source preconditions may fail first.

Select one shared rules file automatically for this repository:

```sh
git config --local ca.rulesFile prompts/rules.md
git ca
git ca pr --source commits
git config --local --unset-all ca.rulesFile # Restore command-specific defaults
```

Precedence is `--rules-file`, repository-local `ca.rulesFile`, command-specific default file, then built-in rules. Only local Git config is queried; global and system settings are ignored, and the last value wins if multiple local values exist. The configured file applies to commits and PR creation/updates with either source. Auth, models, and config commands ignore it without lookup.

Relative configured paths resolve from the current work-tree root, including invocation from a subdirectory or linked worktree. Absolute configured paths work directly. Spaces are preserved, with the same literal-path and UTF-8 content rules as the flag. Configured files bypass default-file reads. Empty or whitespace-only settings, invalid configured files (the same failures listed above), and Git lookup errors stop drafting without fallback; diagnostics identify `ca.rulesFile` and the path and reason where applicable.

When both the flag and local key are absent, commit drafts use `commit-system-prompt.md` and PR drafts use `pr-system-prompt.md` from the config directory above. Each command reads only its own file. Missing files silently use built-in rules. Empty or unreadable files (including invalid UTF-8) warn and fall back to built-in rules.

## Development

See [docs/development.md](docs/development.md) for architecture, runtime flow, release flow, backend caveats, and local development checks.

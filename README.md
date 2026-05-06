# git-ca

`git-ca` is a Git subcommand that drafts commit messages and pull request text using either GitHub Copilot or the OpenAI Codex (ChatGPT) backend. It reads `git diff --cached` for commits, or branch changes for PRs, asks the configured backend for a draft, opens the result in your editor by default, and then lets Git or GitHub CLI finish the action.

## Installation

Option 1 (recommended): Install from crates.io or main stream package managers:

```sh
cargo install git-ca
brew install hankcraft/tap/git-ca
npm install -g @hankcraft/git-ca
bun install -g @hankcraft/git-ca
npx @hankcraft/git-ca --help
bunx @hankcraft/git-ca --help
```


Option 2: Install from this checkout:

```sh
cargo install --path .
```

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

For pull requests:

```sh
git ca pr
```

## Key Features

- Drafts Conventional Commits messages from staged changes.
- Drafts PR title/body text from branch diffs or commit logs and creates PRs with `gh`.
- Supports GitHub Copilot and OpenAI Codex (ChatGPT) backends, with model selection.
- Opens generated text in your editor by default, with `--yes` / `-y` for direct commit or PR creation.
- Supports multiple AI provider accounts, with token persistence and swapping.

## Commands

| Command | Description |
| --- | --- |
| `git ca` | Draft a message for staged changes and run `git commit -e -F <message>` |
| `git ca pr` | Draft a PR title/body from current branch changes and run `gh pr create` |
| `git ca pr --base <branch>` | Compare the current branch against a specific PR base branch |
| `git ca pr --source commits` | Draft PR text from commit messages instead of the branch diff |
| `git ca --model <id>`, `git ca -m <id>` | Use a specific backend model for this command |
| `git ca --yes`, `git ca -y` | Accept generated text without opening the editor; for PRs this creates the PR directly |
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

## Authentication Notes

`git ca auth login` prompts for a backend on a TTY and defaults to Copilot when stdin is piped or running in CI. Copilot supports GitHub device flow or manual token storage with `git ca auth set-token <github-token>`. Codex uses ChatGPT OAuth with a loopback callback on `127.0.0.1:1455` or fallback `:1457`.

## Copilot Request Accounting

GitHub Copilot request accounting depends on both plan and model. GitHub's
documentation is the source of truth because included models and multipliers can
change: <https://docs.github.com/en/copilot/concepts/billing/copilot-requests#model-multipliers>

## Configuration Files

`git-ca` stores configuration under `$XDG_CONFIG_HOME/git-ca` when `XDG_CONFIG_HOME` is set, otherwise under `~/.config/git-ca`:

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

To replace the built-in system prompts, manually create or edit these files:

- `~/.config/git-ca/commit-system-prompt.md` for `git ca`.
- `~/.config/git-ca/pr-system-prompt.md` for `git ca pr`.

When `XDG_CONFIG_HOME` is set, use `$XDG_CONFIG_HOME/git-ca/` instead of `~/.config/git-ca/`. Missing files are ignored. Empty or unreadable files print a warning and fall back to the built-in prompt. Custom PR prompts must still ask the model to return JSON with `title` and `body`.

## Development

See [docs/development.md](docs/development.md) for architecture, runtime flow, release flow, backend caveats, and local development checks.

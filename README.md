# git-ca

`git-ca` is a Git subcommand that drafts commit messages for staged changes using either GitHub Copilot or the OpenAI Codex (ChatGPT) backend. It reads `git diff --cached`, asks the configured backend for a Conventional Commits message, opens the result in Git's normal commit editor, and then lets `git commit` finish the commit.

## Quick Start

Prerequisites:

- Rust toolchain with Cargo
- Git
- Lefthook for Git hooks
- One of: a GitHub account with Copilot access, **or** a ChatGPT account (Plus/Pro/Team) for the Codex backend

Install from this checkout:

```sh
cargo install --path .
install -D -m 0644 docs/man/git-ca.1 ~/.local/share/man/man1/git-ca.1
```

Install from Homebrew after the first release is published:

```sh
brew install hankcraft/tap/git-ca
```

Install from npm or Bun after the first release is published:

```sh
npm install -g @hankcraft/git-ca
bun install -g @hankcraft/git-ca
```

Run without a global install:

```sh
npx @hankcraft/git-ca --help
bunx @hankcraft/git-ca --help
```

The man page install lets Git's own help path resolve `git ca --help`. For the
clap-generated command help, `git ca -h` and `git-ca --help` work directly.

Authenticate with GitHub Copilot (default backend):

```sh
git ca auth login
git ca auth login work
git ca auth use work
```

Or authenticate with the Codex (ChatGPT) backend via PKCE OAuth:

```sh
git ca auth login --provider codex
git ca auth login --provider codex personal
git ca auth use personal
```

The Codex flow opens your browser, completes the same login `codex` itself uses, and stores ChatGPT tokens locally. It needs to bind a loopback callback on `127.0.0.1:1455` (fallback `:1457`).

Or store a GitHub token manually for Copilot (Codex is OAuth-only):

```sh
git ca auth set-token <github-token>
git ca auth set-token --account work <github-token>
```

Create a commit:

```sh
git add <files>
git ca
```

`git-ca` writes a draft commit message, opens your configured Git editor, and passes the edited message to `git commit`. If you save an empty message, Git aborts the commit as usual.

Useful commands:

```sh
git ca auth status
git ca models
git ca config set-model <model-id>
git ca config get-model
git ca --model <model-id>
git ca -m <model-id>
git ca --yes
git ca -y
git ca --auto-accept
git ca --no-verify
```

## Key Features

- Drafts commit messages from the staged diff only.
- Two interchangeable backends: GitHub Copilot or OpenAI Codex (ChatGPT).
- Prompts the active backend to produce Conventional Commits output.
- Opens the generated message in the normal Git commit editor before committing.
- Can commit the generated message directly with `--yes` / `-y` / `--auto-accept` or persisted config.
- Supports per-command model override with `--model`.
- Supports a persisted default model via `git ca config set-model`.
- Supports persisted auto-accept via `git ca config set-auto-accept`.
- Lists chat models available to the authenticated Copilot account; for Codex, prints the known model slugs.
- Copilot login uses GitHub device flow; Codex login uses ChatGPT OAuth (PKCE) with a localhost callback.
- Can store a GitHub token manually for Copilot in environments where device flow is not practical.
- Supports multiple named accounts (mix of Copilot and Codex) with an active-account selector.
- Stores local auth/config files under `$XDG_CONFIG_HOME/git-ca` or `~/.config/git-ca` with restrictive Unix permissions.
- Caches Copilot API tokens and refreshes them when expired or rejected; refreshes ChatGPT access tokens via the rotated refresh token on 401.
- Retries transient backend/network failures with short backoff.
- Applies HTTP connect and request timeouts so stalled endpoints do not hang the CLI indefinitely.

## Commands

| Command | Description |
| --- | --- |
| `git ca` | Draft a message for staged changes and run `git commit -e -F <message>` |
| `git ca --model <id>`, `git ca -m <id>` | Use a specific Copilot model for this commit |
| `git ca --yes`, `git ca -y`, `git ca --auto-accept` | Commit the generated message without opening the editor |
| `git ca --no-verify` | Pass `--no-verify` through to `git commit` |
| `git ca auth login` | Log in with the default backend (Copilot device flow) |
| `git ca auth login <account>` | Log in and store credentials for a named Copilot account |
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
| `git ca config set-auto-accept <true|false>` | Persist whether generated messages commit without opening the editor |
| `git ca config get-auto-accept` | Print the persisted auto-accept setting |

`auth logout` only removes local credentials. Revoke the OAuth grant separately from GitHub account settings if the server-side grant should be invalidated.

## System Architecture

The binary is intentionally small and split by responsibility:

```text
src/main.rs
  CLI dispatch, high-level command orchestration, HTTP client construction

src/cli.rs
  clap argument and subcommand definitions

src/git/
  staged diff reading and editor-backed `git commit` execution

src/commit_msg/
  Copilot prompt construction, diff truncation, and generated-message cleanup

src/auth/
  GitHub device flow, ChatGPT OAuth (PKCE + loopback), local auth file
  storage, Copilot/Codex token exchange/refresh

src/copilot/
  Copilot HTTP client, chat completion calls, model listing, retry/auth wrapper

src/codex/
  Codex HTTP client (chatgpt.com/backend-api/codex), Responses-API request
  builder, SSE event parser, retry/auth wrapper

src/config/
  config/auth paths, JSON persistence, restrictive file/directory permissions

src/error.rs
  typed error model and CLI exit-code mapping
```

Runtime flow for `git ca`:

1. Read the staged diff with `git diff --cached --no-color -U3`.
2. Resolve the active account's backend (Copilot or Codex).
3. Load the persisted default model, unless `--model` was passed; fall back to a backend-specific default (`gpt-4o` for Copilot, `gpt-5.5` for Codex).
4. For Copilot: refresh the Copilot API token from the stored GitHub token. For Codex: use the cached ChatGPT access token, refreshing via `/oauth/token` on 401.
5. Send the chat request with the Conventional Commits prompt — chat-completions for Copilot, Responses-API streamed over SSE for Codex.
6. Strip an accidental outer code fence from the model response.
7. Write `.git/COMMIT_EDITMSG`.
8. Run `git commit -e -F .git/COMMIT_EDITMSG`, optionally with `--no-verify`.
9. If `--yes` / `-y` / `--auto-accept` or `config.auto_accept` is enabled, run `git commit -F .git/COMMIT_EDITMSG` instead so Git commits the generated message directly.

### Codex backend caveat

The Codex backend talks to `https://chatgpt.com/backend-api/codex/responses`, which is the same undocumented endpoint OpenAI's `codex` CLI uses. `git-ca` mimics codex's `originator`/header values to avoid being singled out if OpenAI ever tightens client verification — same posture this project already takes for Copilot, where the request mimics the VS Code Copilot Chat extension. The endpoint can change without notice; if Codex chats start failing with `Codex API …`, expect a follow-up release that tracks the new wire format.

## Copilot Free and Model Multipliers

GitHub Copilot request accounting depends on both plan and model. GitHub's
documentation is the source of truth because included models and multipliers can
change: <https://docs.github.com/en/copilot/concepts/billing/copilot-requests#model-multipliers>

As of the referenced GitHub documentation:

- Copilot Free includes up to 2,000 inline suggestion requests and up to 50 premium requests per month.
- All chat interactions count as premium requests on Copilot Free.
- On paid Copilot plans, GPT-5 mini, GPT-4.1, and GPT-4o are included models and have a 0 premium-request multiplier.
- On Copilot Free, GPT-5 mini, GPT-4.1, and GPT-4o each consume 1 premium request.
- Other premium model multipliers vary by model and may be unavailable on Copilot Free.

## Development Flow

Enable the project Git hooks once per checkout:

```sh
lefthook install
```

Run the standard checks before committing:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

During local development, `cargo test` uses `wiremock` and needs permission to bind local ports.

Recommended change flow:

1. Keep changes focused and match the existing module boundaries.
2. Add or update tests for behavior changes and bug fixes.
3. Run `cargo fmt` before final verification.
4. Run the full check set above.
5. Commit related changes atomically with an informative Conventional Commits message.

## Release Flow

Releases are built by cargo-dist and can be published either from GitHub Actions or from a local maintainer machine.

### GitHub Actions release

Push a version tag to publish GitHub Release artifacts, crates.io, Homebrew, and npm from CI.

### Bump version

Use `cargo-release` to bump `Cargo.toml`, refresh `Cargo.lock`, update the manual page header, create the version commit, and create the matching `v*` tag. The project release settings live in `release.toml`.

Install the release helper once:

```sh
cargo install cargo-release
```

Preview a patch release without writing changes:

```sh
cargo release patch
```

Create the version commit and tag:

```sh
cargo release patch --execute
```

Use `minor`, `major`, or an exact SemVer version when needed:

```sh
cargo release minor --execute
cargo release major --execute
cargo release 0.2.0 --execute
```

`cargo-release` runs `cargo check` before committing so `Cargo.lock` is included when the package version changes. It also updates `docs/man/git-ca.1` from this template:

```roff
.TH GIT-CA 1 "{{date}}" "git-ca {{version}}" "User Commands"
```

The generated tag uses `v{{version}}`, which is the tag format cargo-dist uses for GitHub Actions releases. `release.toml` keeps `publish = false` and `push = false` so publishing stays in GitHub Actions and the maintainer explicitly pushes the release tag.

Do not reuse a version already published to crates.io or npm. Published package versions are immutable.

Release checklist:

1. Run `cargo release patch`, `cargo release minor`, or `cargo release <version>` and inspect the dry-run output.
2. Run `cargo release patch --execute`, `cargo release minor --execute`, or `cargo release <version> --execute`.
3. Run `cargo fmt --check`.
4. Run `cargo clippy --all-targets --all-features -- -D warnings`.
5. Run `cargo test`.
6. Run `cargo publish --dry-run --locked`.
7. Run `dist plan --allow-dirty`.
8. Build the release binary locally if you want an extra smoke test:

```sh
cargo build --release
target/release/git-ca --help
target/release/git-ca auth --help
target/release/git-ca config --help
```

9. Push the version commit and matching tag:

```sh
git push origin main
git push origin v0.2.0
```

The release workflow uploads archives and checksums to GitHub Releases, then runs cargo-dist custom publish jobs for crates.io, Homebrew, and npm. Keep these jobs configured through `dist-workspace.toml` instead of editing the generated `.github/workflows/release.yml` directly:

```toml
publish-jobs = ["./publish-homebrew", "./publish-npm", "./publish-crates"]
```

`dist init` may warn that the built-in Homebrew publish job is disabled. That is expected because `.github/workflows/publish-homebrew.yml` owns Homebrew publishing so the tap commit author can be customized.

Configure these repository secrets before pushing release tags:

- `CARGO_REGISTRY_TOKEN` with publish access to the `git-ca` crate on crates.io.
- `HOMEBREW_TAP_TOKEN` with write access to `hankcraft/homebrew-tap`.

The generated release workflow uses GitHub Actions' automatic `GITHUB_TOKEN` for GitHub Releases. Do not add a repository secret named `GITHUB_TOKEN`.

npm publishing uses npm Trusted Publishing with GitHub Actions OIDC, not `NPM_TOKEN`. Configure the npm package trusted publisher on npmjs.com with:

- Provider: GitHub Actions
- Organization or user: `hankcraft`
- Repository: `git-ca`
- Workflow filename: `release.yml`
- Environment name: unset unless the workflow is changed to use a GitHub environment

The publish command lives in the reusable `.github/workflows/publish-npm.yml` workflow, but cargo-dist calls it from `.github/workflows/release.yml`. npm validates the calling workflow for `workflow_call` publishes, so the trusted publisher must use `release.yml`.

### Local release

Use the local release script when you want to publish from your own machine instead of relying on GitHub Actions. It defaults to a dry run and requires `--execute` before creating or publishing anything.

Dry-run the local release checks:

```sh
scripts/release-local.sh
```

Publish all local release channels:

```sh
scripts/release-local.sh --execute --homebrew-tap-dir ../homebrew-tap
```

Skip one channel if it was already published or should stay manual:

```sh
scripts/release-local.sh --execute --homebrew-tap-dir ../homebrew-tap --skip npm
scripts/release-local.sh --execute --homebrew-tap-dir ../homebrew-tap --skip homebrew
scripts/release-local.sh --execute --homebrew-tap-dir ../homebrew-tap --skip crates
```

Local publishing requirements:

- GitHub Release hosting: `GH_TOKEN` or an authenticated cargo-dist/GitHub CLI setup with write access to `hankcraft/git-ca`.
- crates.io: `cargo login` or `CARGO_REGISTRY_TOKEN` with publish access to `git-ca`.
- npm: `npm login`, `.npmrc`, or `NODE_AUTH_TOKEN` with publish access to `@hankcraft/git-ca`.
- Homebrew: a clean local checkout of `hankcraft/homebrew-tap` with push access, passed with `--homebrew-tap-dir` or `HOMEBREW_TAP_DIR`.

Copy `.env.example` to `.env` if you want a local template for release-related environment variables. Do not commit `.env`.

cargo-dist currently builds `git-ca` for `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, and `aarch64-unknown-linux-gnu`, so the Homebrew formula supports macOS and Linuxbrew on x86_64 and aarch64 Linux.

Production Homebrew releases install `docs/man/git-ca.1` automatically, so `git ca --help` can resolve Git's manual page after `brew install hankcraft/tap/git-ca`.

## Configuration Files

`git-ca` stores configuration under `$XDG_CONFIG_HOME/git-ca` when `XDG_CONFIG_HOME` is set, otherwise under `~/.config/git-ca`:

```text
~/.config/git-ca/config.json
~/.config/git-ca/auth.json
```

On Unix, the config directory is set to `0700` and JSON files are written with `0600` permissions.

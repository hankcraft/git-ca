# git-ca

`git-ca` is a Git subcommand that drafts commit messages for staged changes using GitHub Copilot. It reads `git diff --cached`, asks Copilot for a Conventional Commits message, opens the result in Git's normal commit editor, and then lets `git commit` finish the commit.

## Quick Start

Prerequisites:

- Rust toolchain with Cargo
- Git
- Lefthook for Git hooks
- A GitHub account with Copilot access

Install from this checkout:

```sh
cargo install --path .
install -D -m 0644 docs/man/git-ca.1 ~/.local/share/man/man1/git-ca.1
```

Install from Homebrew after the first release is published:

```sh
brew install hankcraft/tap/git-ca
```

The man page install lets Git's own help path resolve `git ca --help`. For the
clap-generated command help, `git ca -h` and `git-ca --help` work directly.

Authenticate with GitHub Copilot:

```sh
git ca auth login
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
- Prompts Copilot to produce Conventional Commits output.
- Opens the generated message in the normal Git commit editor before committing.
- Can commit the generated message directly with `--yes` / `-y` / `--auto-accept` or persisted config.
- Supports per-command model override with `--model`.
- Supports a persisted default model via `git ca config set-model`.
- Supports persisted auto-accept via `git ca config set-auto-accept`.
- Lists chat models available to the authenticated Copilot account.
- Uses GitHub device flow for login.
- Stores local auth/config files under the platform config directory with restrictive Unix permissions.
- Caches Copilot API tokens and refreshes them when expired or rejected.
- Retries transient Copilot/network failures with short backoff.
- Applies HTTP connect and request timeouts so stalled endpoints do not hang the CLI indefinitely.

## Commands

| Command | Description |
| --- | --- |
| `git ca` | Draft a message for staged changes and run `git commit -e -F <message>` |
| `git ca --model <id>`, `git ca -m <id>` | Use a specific Copilot model for this commit |
| `git ca --yes`, `git ca -y`, `git ca --auto-accept` | Commit the generated message without opening the editor |
| `git ca --no-verify` | Pass `--no-verify` through to `git commit` |
| `git ca auth login` | Log in with GitHub device flow |
| `git ca auth logout` | Delete locally stored tokens |
| `git ca auth status` | Show local auth state and cached Copilot token TTL |
| `git ca models` | List available Copilot chat models |
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
  GitHub device flow, local auth file storage, Copilot token exchange/refresh

src/copilot/
  Copilot HTTP client, chat completion calls, model listing, retry/auth wrapper

src/config/
  config/auth paths, JSON persistence, restrictive file/directory permissions

src/error.rs
  typed error model and CLI exit-code mapping
```

Runtime flow for `git ca`:

1. Read the staged diff with `git diff --cached --no-color -U3`.
2. Load the persisted default model, unless `--model` was passed.
3. Load or refresh the Copilot API token from the stored GitHub token.
4. Send a chat completion request with the Conventional Commits prompt.
5. Strip an accidental outer code fence from the model response.
6. Write `.git/COMMIT_EDITMSG`.
7. Run `git commit -e -F .git/COMMIT_EDITMSG`, optionally with `--no-verify`.
8. If `--yes` / `-y` / `--auto-accept` or `config.auto_accept` is enabled, run `git commit -F .git/COMMIT_EDITMSG` instead so Git commits the generated message directly.

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

Releases are built by cargo-dist and published from GitHub Actions when a version tag is pushed.

1. Update `version` in `Cargo.toml`.
2. Run `cargo fmt --check`.
3. Run `cargo clippy --all-targets --all-features -- -D warnings`.
4. Run `cargo test`.
5. Run `dist plan`.
6. Build the release binary locally if you want an extra smoke test:

```sh
cargo build --release
target/release/git-ca --help
target/release/git-ca auth --help
target/release/git-ca config --help
```

7. Commit the version change and create a version tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release workflow uploads archives and checksums to GitHub Releases, then publishes the Homebrew formula to `hankcraft/homebrew-tap`. Configure a `HOMEBREW_TAP_TOKEN` GitHub secret in this repository with write access to that tap before pushing release tags.

cargo-dist currently builds `git-ca` for `x86_64-apple-darwin`, `aarch64-apple-darwin`, and `x86_64-unknown-linux-gnu`, so the Homebrew formula supports macOS and Linuxbrew on x86_64 Linux.

## Configuration Files

`git-ca` uses the platform config directory reported by the `directories` crate. On Linux this is typically:

```text
~/.config/git-ca/config.json
~/.config/git-ca/auth.json
```

On Unix, the config directory is set to `0700` and JSON files are written with `0600` permissions.

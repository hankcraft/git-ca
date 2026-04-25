# git-ca

`git-ca` is a Git subcommand that drafts commit messages for staged changes using GitHub Copilot. It reads `git diff --cached`, asks Copilot for a Conventional Commits message, opens the result in Git's normal commit editor, and then lets `git commit` finish the commit.

## Quick Start

Prerequisites:

- Rust toolchain with Cargo
- Git
- A GitHub account with Copilot access

Install from this checkout:

```sh
cargo install --path .
```

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
git ca --no-verify
```

## Key Features

- Drafts commit messages from the staged diff only.
- Prompts Copilot to produce Conventional Commits output.
- Opens the generated message in the normal Git commit editor before committing.
- Supports per-command model override with `--model`.
- Supports a persisted default model via `git ca config set-model`.
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
| `git ca --model <id>` | Use a specific Copilot model for this commit |
| `git ca --no-verify` | Pass `--no-verify` through to `git commit` |
| `git ca auth login` | Log in with GitHub device flow |
| `git ca auth logout` | Delete locally stored tokens |
| `git ca auth status` | Show local auth state and cached Copilot token TTL |
| `git ca models` | List available Copilot chat models |
| `git ca config set-model <id>` | Persist the default model |
| `git ca config get-model` | Print the persisted default model |

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

## Development Flow

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

There is no release automation in this repository yet. A manual release should use this checklist:

1. Update `version` in `Cargo.toml`.
2. Run `cargo fmt --check`.
3. Run `cargo clippy --all-targets --all-features -- -D warnings`.
4. Run `cargo test`.
5. Build the release binary with `cargo build --release`.
6. Smoke test the binary:

   ```sh
   target/release/git-ca --help
   target/release/git-ca auth --help
   target/release/git-ca config --help
   ```

7. Commit the version change and tag the release.

## Configuration Files

`git-ca` uses the platform config directory reported by the `directories` crate. On Linux this is typically:

```text
~/.config/git-ca/config.json
~/.config/git-ca/auth.json
```

On Unix, the config directory is set to `0700` and JSON files are written with `0600` permissions.

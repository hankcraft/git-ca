# Development

Developer-facing notes for `git-ca`.

## System Architecture

The binary is intentionally small and split by responsibility:

```text
src/main.rs
  CLI dispatch, high-level command orchestration, HTTP client construction

src/cli.rs
  clap argument and subcommand definitions

src/git/
  staged diff/branch PR source reading, editor-backed `git commit`, and `gh pr create` execution

src/commit_msg/
  Copilot prompt construction, diff truncation, and generated-message cleanup

src/pr_msg/
  PR prompt construction, source truncation, and generated JSON parsing

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

## Runtime Flow

Runtime flow for `git ca`:

1. Read the staged diff with `git diff --cached --no-color -U3`.
2. Resolve the active account's backend (Copilot or Codex).
3. Load the persisted default model, unless `--model` was passed; fall back to a backend-specific default (`gpt-4o` for Copilot, `gpt-5.5` for Codex).
4. For Copilot: refresh the Copilot API token from the stored GitHub token. For Codex: use the cached ChatGPT access token, refreshing via `/oauth/token` on 401.
5. Load `$XDG_CONFIG_HOME/git-ca/commit-system-prompt.md` or `~/.config/git-ca/commit-system-prompt.md` if present and non-empty, then use it to replace only the built-in prompt's `Rules` section; otherwise use the full built-in Conventional Commits prompt.
6. Send the chat request - chat-completions for Copilot, Responses-API streamed over SSE for Codex.
7. Strip an accidental outer code fence from the model response.
8. Write `.git/COMMIT_EDITMSG`.
9. Run `git commit -e -F .git/COMMIT_EDITMSG`, optionally with `--no-verify`.
10. If `--yes` / `-y` or `config.auto_accept` is enabled, run `git commit -F .git/COMMIT_EDITMSG` instead so Git commits the generated message directly.

Runtime flow for `git ca pr`:

1. Resolve the base branch from `--base`, else `origin/HEAD`, else `main`.
2. Resolve `git merge-base <base> HEAD`.
3. Read either `git diff --no-color -U3 <merge-base>...HEAD` or `git log --no-merges --format=%s%n%n%b <merge-base>..HEAD`.
4. Resolve the active account's backend and model the same way `git ca` does.
5. Load `$XDG_CONFIG_HOME/git-ca/pr-system-prompt.md` or `~/.config/git-ca/pr-system-prompt.md` if present and non-empty, then use it to replace only the built-in prompt's `Rules` section; otherwise use the full built-in PR prompt.
6. Ask the backend for compact JSON containing `title` and `body`.
7. Parse and validate the generated PR text.
8. Unless `--yes` / `-y` or `config.auto_accept_pr` is enabled, write `.git/PULL_REQUEST_EDITMSG`, open the configured Git editor, and read back the edited title/body.
9. Write `.git/PULL_REQUEST_BODY` and run `gh pr create --base <base> --title <title> --body-file <path>`.

## Codex Backend Caveat

The Codex backend talks to `https://chatgpt.com/backend-api/codex/responses`, which is the same undocumented endpoint OpenAI's `codex` CLI uses. `git-ca` mimics codex's `originator`/header values to avoid being singled out if OpenAI ever tightens client verification - same posture this project already takes for Copilot, where the request mimics the VS Code Copilot Chat extension. The endpoint can change without notice; if Codex chats start failing with `Codex API ...`, expect a follow-up release that tracks the new wire format.

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

### GitHub Actions Release

Push a version tag to publish GitHub Release artifacts, crates.io, Homebrew, and npm from CI.

### Bump Version

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

### Local Release

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

#!/usr/bin/env bash
set -euo pipefail

installer="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/install-dev.sh"
sandbox="$(mktemp -d "${TMPDIR:-/tmp}/git-ca-dev-test.XXXXXX")"
trap 'rm -rf "$sandbox"' EXIT
export GIT_BIN="$(command -v git)"
export TMPDIR="$sandbox/tmp"
mkdir -p "$TMPDIR" "$sandbox/repo/src" "$sandbox/repo/scripts" "$sandbox/bin"
cd "$sandbox/repo"
cp "$installer" scripts/
printf '/target\n' > .gitignore
cat > Cargo.toml <<'TOML'
[package]
name = "git-ca"
version = "0.2.6"
edition = "2021"
TOML
printf 'fn main() { println!("git-ca {}", env!("CARGO_PKG_VERSION")); }\n' > src/main.rs
touch deleted-source.rs
cargo generate-lockfile --offline
git init -q
git config core.hooksPath /dev/null
git add .
git -c user.name=Test -c user.email=test@example.com commit -qm fixture
manifest_before="$(cat Cargo.toml Cargo.lock)"

scripts/install-dev.sh --root "$sandbox/install" --offline --debug
expected="0.2.6-dev+$(git rev-parse --short HEAD)"
[[ "$("$sandbox/install/bin/git-ca")" == "git-ca $expected" ]]
[[ "$(cargo install --list --root "$sandbox/install")" == *"git-ca v$expected "* ]]
[[ "$(cat Cargo.toml Cargo.lock)" == "$manifest_before" ]]
[[ -z "$(git status --porcelain)" ]]
[[ -z "$(ls -A "$TMPDIR")" ]]

# Untracked modules must be installed; deleted tracked files must stay deleted.
printf 'pub const LABEL: &str = "local";\n' > src/local.rs
printf 'mod local; fn main() { println!("git-ca {} {}", env!("CARGO_PKG_VERSION"), local::LABEL); }\n' > src/main.rs
rm deleted-source.rs
status_before="$(git status --porcelain)"
cat > "$sandbox/bin/git" <<'SH'
#!/usr/bin/env bash
if [[ "$*" == "rev-parse --short HEAD" ]]; then
  printf '0123456\n'
else
  exec "$GIT_BIN" "$@"
fi
SH
chmod +x "$sandbox/bin/git"
export PATH="$sandbox/bin:$PATH"
scripts/install-dev.sh --root "$sandbox/install" --offline --debug
expected="0.2.6-dev+0123456.dirty"
[[ "$("$sandbox/install/bin/git-ca")" == "git-ca $expected local" ]]
[[ "$(cargo install --list --root "$sandbox/install")" == *"git-ca v$expected "* ]]
[[ "$(cat Cargo.toml Cargo.lock)" == "$manifest_before" ]]
[[ "$(git status --porcelain)" == "$status_before" ]]
[[ -z "$(ls -A "$TMPDIR")" ]]

# Failed builds must clean up and leave checkout state intact too.
printf '#!/usr/bin/env bash\nexit 42\n' > "$sandbox/bin/cargo"
chmod +x "$sandbox/bin/cargo"
exit_code=0
scripts/install-dev.sh --offline || exit_code=$?
[[ "$exit_code" == 42 ]]
[[ "$(cat Cargo.toml Cargo.lock)" == "$manifest_before" ]]
[[ "$(git status --porcelain)" == "$status_before" ]]
[[ -z "$(ls -A "$TMPDIR")" ]]
printf 'install-dev checks passed\n'

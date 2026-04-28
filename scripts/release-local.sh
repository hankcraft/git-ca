#!/usr/bin/env bash
set -euo pipefail

readonly SCRIPT_NAME="$(basename "$0")"

execute=false
allow_dirty=false
tag=""
dist_bin="${DIST_BIN:-dist}"
homebrew_tap_dir="${HOMEBREW_TAP_DIR:-}"
skip_github=false
skip_crates=false
skip_npm=false
skip_homebrew=false

usage() {
  cat <<'USAGE'
Usage: scripts/release-local.sh [options]

Runs local release checks by default. Pass --execute to publish.

Options:
  --execute                    Publish instead of dry-running checks.
  --allow-dirty                Allow releasing with uncommitted changes.
  --tag <tag>                  Release tag to pass to cargo-dist. Defaults to v<Cargo.toml version>.
  --dist-bin <path>            cargo-dist executable. Defaults to DIST_BIN or dist.
  --homebrew-tap-dir <path>    Local authenticated checkout of hankcraft/homebrew-tap.
  --skip <channel>             Skip github, crates, npm, or homebrew. May be repeated.
  -h, --help                   Show this help.

Required credentials for --execute:
  GitHub release hosting: GH_TOKEN or an authenticated gh/cargo-dist setup.
  crates.io: cargo login or CARGO_REGISTRY_TOKEN.
  npm: npm login, .npmrc, or NODE_AUTH_TOKEN.
  Homebrew: --homebrew-tap-dir pointing at a clean tap checkout with push access.
USAGE
}

log() {
  printf '[%s] %s\n' "$SCRIPT_NAME" "$*"
}

fail() {
  printf '[%s] error: %s\n' "$SCRIPT_NAME" "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

run() {
  log "+ $*"
  "$@"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --execute)
        execute=true
        shift
        ;;
      --allow-dirty)
        allow_dirty=true
        shift
        ;;
      --tag)
        [[ $# -ge 2 ]] || fail "--tag requires a value"
        tag="$2"
        shift 2
        ;;
      --dist-bin)
        [[ $# -ge 2 ]] || fail "--dist-bin requires a value"
        dist_bin="$2"
        shift 2
        ;;
      --homebrew-tap-dir)
        [[ $# -ge 2 ]] || fail "--homebrew-tap-dir requires a value"
        homebrew_tap_dir="$2"
        shift 2
        ;;
      --skip)
        [[ $# -ge 2 ]] || fail "--skip requires a channel"
        case "$2" in
          github) skip_github=true ;;
          crates) skip_crates=true ;;
          npm) skip_npm=true ;;
          homebrew) skip_homebrew=true ;;
          *) fail "unknown channel for --skip: $2" ;;
        esac
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail "unknown option: $1"
        ;;
    esac
  done
}

package_field() {
  local field="$1"
  awk -v field="$field" '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && in_package { exit }
    in_package && $1 == field { gsub(/"/, "", $3); print $3; exit }
  ' Cargo.toml
}

ensure_clean_tree() {
  if [[ "$allow_dirty" == true ]]; then
    log "allowing dirty working tree"
    return
  fi

  if [[ -n "$(git status --porcelain)" ]]; then
    fail "working tree is dirty; commit/stash changes or pass --allow-dirty"
  fi
}

run_checks() {
  run cargo fmt --check
  run cargo clippy --all-targets --all-features -- -D warnings
  run cargo test
  local cargo_publish_args=(publish --dry-run --locked)
  if [[ "$allow_dirty" == true ]]; then
    cargo_publish_args+=(--allow-dirty)
  fi
  run cargo "${cargo_publish_args[@]}"
  run "$dist_bin" plan --allow-dirty --tag "$tag"
}

build_dist_artifacts() {
  run "$dist_bin" build --allow-dirty --artifacts=all --tag "$tag"
}

host_github_release() {
  [[ "$skip_github" == false ]] || return
  run "$dist_bin" host --allow-dirty --steps=create --tag "$tag"
  run "$dist_bin" host --allow-dirty --steps=upload --steps=release --tag "$tag"
}

publish_crates() {
  [[ "$skip_crates" == false ]] || return
  local cargo_publish_args=(publish --locked)
  if [[ "$allow_dirty" == true ]]; then
    cargo_publish_args+=(--allow-dirty)
  fi
  run cargo "${cargo_publish_args[@]}"
}

find_one_artifact() {
  local pattern="$1"
  local description="$2"
  shopt -s nullglob
  local matches=(target/distrib/$pattern)
  shopt -u nullglob

  [[ ${#matches[@]} -gt 0 ]] || fail "no $description found in target/distrib"
  [[ ${#matches[@]} -eq 1 ]] || fail "expected one $description, found ${#matches[@]}"
  printf '%s\n' "${matches[0]}"
}

publish_npm() {
  [[ "$skip_npm" == false ]] || return
  local package
  package="$(find_one_artifact '*-npm-package.tar.gz' 'npm package')"
  run npm publish --access public "$package"
}

patch_homebrew_formula() {
  local formula_path="$1"
  ruby -0pi -e 'sub(%Q{    install_binary_aliases!\n\n    # Homebrew}, %Q{    install_binary_aliases!\n    man1.install "git-ca.1" if File.exist?("git-ca.1")\n\n    # Homebrew}); sub(%Q{    leftover_contents = Dir["*"] - doc_files\n}, %Q{    leftover_contents = Dir["*"] - doc_files\n    leftover_contents -= ["git-ca.1"]\n})' "$formula_path"
}

publish_homebrew() {
  [[ "$skip_homebrew" == false ]] || return
  [[ -n "$homebrew_tap_dir" ]] || fail "--homebrew-tap-dir is required for Homebrew publishing"
  [[ -d "$homebrew_tap_dir/.git" ]] || fail "Homebrew tap dir is not a git checkout: $homebrew_tap_dir"
  [[ -z "$(git -C "$homebrew_tap_dir" status --porcelain)" ]] || fail "Homebrew tap checkout is dirty: $homebrew_tap_dir"

  local formula
  formula="$(find_one_artifact '*.rb' 'Homebrew formula')"

  mkdir -p "$homebrew_tap_dir/Formula"
  run cp "$formula" "$homebrew_tap_dir/Formula/"

  local formula_file
  formula_file="$homebrew_tap_dir/Formula/$(basename "$formula")"
  patch_homebrew_formula "$formula_file"

  if command -v brew >/dev/null 2>&1; then
    run brew style --except-cops FormulaAudit/Homepage,FormulaAudit/Desc,FormulaAuditStrict --fix "$formula_file"
  else
    log "brew not found; skipping brew style"
  fi

  if git -C "$homebrew_tap_dir" diff --quiet -- "Formula/$(basename "$formula")"; then
    log "Homebrew formula unchanged; skipping commit and push"
    return
  fi

  run git -C "$homebrew_tap_dir" add "Formula/$(basename "$formula")"
  run git -C "$homebrew_tap_dir" commit -m "git-ca ${version}"
  run git -C "$homebrew_tap_dir" push
}

main() {
  parse_args "$@"

  require_command cargo
  require_command git
  require_command "$dist_bin"

  if [[ "$execute" == true && "$skip_npm" == false ]]; then
    require_command npm
  fi

  if [[ "$execute" == true && "$skip_homebrew" == false ]]; then
    require_command ruby
  fi

  name="$(package_field name)"
  version="$(package_field version)"
  [[ -n "$name" ]] || fail "could not read package name from Cargo.toml"
  [[ -n "$version" ]] || fail "could not read package version from Cargo.toml"

  if [[ -z "$tag" ]]; then
    tag="v${version}"
  fi

  log "package: ${name} ${version}"
  log "tag: ${tag}"

  ensure_clean_tree
  run_checks

  if [[ "$execute" == false ]]; then
    log "dry run complete; pass --execute to build, host, and publish release artifacts"
    return
  fi

  build_dist_artifacts
  host_github_release
  publish_crates
  publish_npm
  publish_homebrew
  log "local release complete"
}

main "$@"

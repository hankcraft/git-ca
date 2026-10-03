#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
repo_dir="$PWD"
base_version="$(awk '
  /^\[package\]$/ { in_package = 1; next }
  /^\[/ && in_package { exit }
  in_package && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
' Cargo.toml)"
dev_version="${base_version%%+*}-dev+$(git rev-parse --short HEAD)"
if [[ -n "$(git status --porcelain)" ]]; then
  dev_version+=".dirty"
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/git-ca-dev.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
source_dir="$tmp_dir/source"
mkdir "$source_dir"

# Copy working-tree contents, including untracked source, but skip ignored files.
git ls-files --cached --others --exclude-standard -z > "$tmp_dir/files"
while IFS= read -r -d '' file; do
  [[ -e "$file" || -L "$file" ]] || continue
  mkdir -p "$source_dir/$(dirname "$file")"
  cp -P "$file" "$source_dir/$file"
done < "$tmp_dir/files"
rm "$tmp_dir/files"

for file in Cargo.toml Cargo.lock; do
  awk -v version="$dev_version" '
    /^name = "git-ca"$/ { in_package = 1 }
    in_package && /^version = / {
      $0 = "version = \"" version "\""
      in_package = 0
    }
    { print }
  ' "$source_dir/$file" > "$source_dir/$file.new"
  mv "$source_dir/$file.new" "$source_dir/$file"
done

printf '[install-dev.sh] installing git-ca %s\n' "$dev_version"
cargo install --path "$source_dir" --locked --force --target-dir "$repo_dir/target" "$@"

#!/usr/bin/env bash
set -euo pipefail

node_modules/.bin/tauri "$@"
[[ ${1:-} == build ]] || exit 0

target=
while (( $# )); do
  if [[ $1 == --target ]]; then target=$2; break; fi
  shift
done
bundle="src-tauri/target/${target:+$target/}release/bundle"
tmp=$(mktemp -d)
mkdir "$tmp/dmg" "$tmp/updater"
attached=false
cleanup() {
  if $attached; then hdiutil detach "$tmp/dmg" || return; fi
  rm -rf "$tmp"
}
trap cleanup EXIT
hdiutil attach -readonly -nobrowse -mountpoint "$tmp/dmg" "$bundle"/dmg/*.dmg
attached=true
codesign --verify --deep --strict --verbose=2 "$tmp/dmg/Codex Tools.app"
hdiutil detach "$tmp/dmg"
attached=false
tar -xzf "$bundle"/macos/*.app.tar.gz -C "$tmp/updater"
codesign --verify --deep --strict --verbose=2 "$tmp/updater/Codex Tools.app"

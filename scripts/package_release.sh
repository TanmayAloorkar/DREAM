#!/usr/bin/env bash
# Build the release archive: every tracked file at HEAD, plus the desktop app at the top level
# as Run_Dream, so users extract the folder and double-click Run_Dream to start.
# Usage: scripts/package_release.sh <version> [path to dream-gui binary]
set -euo pipefail

version=${1:?usage: scripts/package_release.sh <version> [binary]}
binary=${2:-gui/target/release/dream-gui}
name="DREAM-$version"
out=${OUT_DIR:-dist}
archive="$name-linux-x86_64.tar.gz"

stage=$(mktemp -d)
trap 'rm -rf "${stage:?}"' EXIT
git archive --format=tar --prefix="$name/" HEAD | tar -x -C "$stage"
install -m 755 "$binary" "$stage/$name/Run_Dream"

mkdir -p "$out"
tar -C "$stage" -czf "$out/$archive" "$name"
(cd "$out" && sha256sum "$archive" > "$archive.sha256")
echo "$out/$archive"

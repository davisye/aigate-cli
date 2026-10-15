#!/usr/bin/env bash
set -euo pipefail

[[ $# == 3 || $# == 4 ]] || { printf 'usage: bash scripts/package-binary.sh TARGET VERSION SHA [ARCHIVE_LABEL]\n' >&2; exit 1; }
target=$1
version=$2
sha=$3
archive_label=${4:-nightly}
[[ $archive_label == nightly || ( $archive_label == "$version" && $archive_label =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(beta|rc)\.(0|[1-9][0-9]*))?$ ) ]] || { printf 'Invalid archive label\n' >&2; exit 1; }
case "$target" in
    x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|aarch64-apple-darwin) ;;
    *) printf 'Unsupported Unix target: %s\n' "$target" >&2; exit 1 ;;
esac
binary="${CARGO_TARGET_DIR:-target}/$target/release/aigate-cli"
[[ $("$binary" --version) == "aigate-cli $version" ]]
"$binary" --help
"$binary" regions --json

mkdir -p dist
stage=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/aigate-binary.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
cp "$binary" "$stage/aigate-cli"
cp LICENSE THIRD_PARTY_NOTICES.md README.md CHANGELOG.md "$stage/"
printf 'Version: %s\nCommit: %s\nTarget: %s\n' "$version" "$sha" "$target" > "$stage/BUILD-INFO.txt"
archive="$PWD/dist/aigate-cli-$archive_label-$target.tar.gz"
tar -czf "$archive" -C "$stage" aigate-cli LICENSE THIRD_PARTY_NOTICES.md README.md CHANGELOG.md BUILD-INFO.txt
mkdir "$stage/unpacked"
tar -xzf "$archive" -C "$stage/unpacked"
[[ $("$stage/unpacked/aigate-cli" --version) == "aigate-cli $version" ]]

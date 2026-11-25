#!/usr/bin/env bash
set -euo pipefail

die() { printf 'Linux packaging failed: %s\n' "$*" >&2; exit 1; }
[[ $# == 2 ]] || die 'usage: bash scripts/package-linux.sh TARGET VERSION'
target=$1
version=$2
[[ $version =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(alpha|beta|rc)\.(0|[1-9][0-9]*))?$ ]] ||
    die "invalid release version: $version"

case "$target" in
    x86_64-unknown-linux-gnu) package_arch=amd64; linux_arch=x86_64 ;;
    aarch64-unknown-linux-gnu) package_arch=arm64; linux_arch=aarch64 ;;
    *) die "unsupported Linux target: $target" ;;
esac

nfpm=${NFPM_BIN:-nfpm}
command -v "$nfpm" >/dev/null || die 'nFPM is required'
prefix="aigate-cli-$version"
archive="$PWD/dist/$prefix-$target.tar.gz"
[[ -s $archive ]] || die "missing binary archive: $archive"
stage=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/aigate-linux.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT

mkdir -p "$stage/docs" "$stage/packages"
tar -xzf "$archive" -C "$stage" aigate-cli LICENSE THIRD_PARTY_NOTICES.md README.md CHANGELOG.md BUILD-INFO.txt
grep -Fqx "Version: $version" "$stage/BUILD-INFO.txt" || die 'binary archive version mismatch'
grep -Fqx "Target: $target" "$stage/BUILD-INFO.txt" || die 'binary archive target mismatch'
install -m 644 "$stage"/{LICENSE,THIRD_PARTY_NOTICES.md,README.md,CHANGELOG.md,BUILD-INFO.txt} "$stage/docs/"

export AIGATE_PACKAGE_ARCH="$package_arch" AIGATE_PACKAGE_STAGE="$stage"
export AIGATE_PACKAGE_VERSION="${version/-/'~'}"
pack() {
    "$nfpm" package --config packaging/nfpm.yaml --packager "$1" --target "$stage/packages/$2"
    [[ -s $stage/packages/$2 ]] || die "empty package: $2"
}

pack deb "$prefix-$package_arch.deb"
pack rpm "$prefix-$linux_arch.rpm"
# Pacman sorts an attached prerelease suffix before the corresponding stable version.
AIGATE_PACKAGE_VERSION="${version/-/}" pack archlinux "$prefix-$linux_arch.pkg.tar.zst"
mv -- "$stage"/packages/* dist/

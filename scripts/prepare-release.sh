#!/usr/bin/env bash
set -euo pipefail

die() { printf 'Release preparation failed: %s\n' "$*" >&2; exit 1; }

check_only=false
if [[ ${1:-} == --check ]]; then
    check_only=true
    shift
fi
[[ $# == 1 ]] || die 'usage: bash scripts/prepare-release.sh [--check] vX.Y.Z[-beta.N]'
tag=$1
[[ $tag =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-beta\.(0|[1-9][0-9]*))?$ ]] || die 'expected vX.Y.Z or vX.Y.Z-beta.N'

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
[[ $(git rev-parse --show-toplevel) == "$(pwd -P)" ]] || die 'run from the project Git repository'
[[ -z $(git status --porcelain --untracked-files=all) ]] || die 'commit or remove pending changes first'
commit=$(git rev-parse --verify "refs/tags/$tag^{commit}") || die 'release tag does not exist'
[[ $(git rev-parse HEAD) == "$commit" ]] || die 'HEAD must match the release tag'
main_ref=refs/remotes/origin/main
git show-ref --verify --quiet "$main_ref" || main_ref=refs/heads/main
git merge-base --is-ancestor "$commit" "$main_ref" || die 'release commit must belong to main'
version=$(cargo metadata --locked --offline --no-deps --format-version 1 | jq -er '.packages[] | select(.name == "aigate-cli") | .version')
[[ $tag == "v$version" ]] || die 'tag and Cargo package version differ'

for file in Cargo.toml Cargo.lock LICENSE README.md THIRD_PARTY_NOTICES.md CHANGELOG.md; do
    git cat-file -e "$commit:$file" || die "missing tracked release file: $file"
done
# Only finalized version notes are accepted; unreleased work must not be tagged.
notes=$(awk -v heading="## [$version]" '
    index($0, heading) == 1 { found=1; next }
    found && /^## / { exit }
    found { print }
    END { if (!found) exit 1 }
' CHANGELOG.md) || die 'add versioned release notes to CHANGELOG.md before tagging'
[[ $notes =~ [^[:space:]] ]] || die 'versioned release notes are empty'

if "$check_only"; then
    printf 'Validated %s at %s\n' "$tag" "$commit"
    exit 0
fi

mkdir -p dist
archive="aigate-cli-$version-source.tar.gz"
[[ ! -e dist/$archive && ! -e dist/SHA256SUMS && ! -e dist/RELEASE_NOTES.md ]] || die 'dist contains existing release outputs'
git archive --format=tar --prefix="aigate-cli-$version/" "$commit" | gzip -n > "dist/$archive"
members=$(tar -tzf "dist/$archive")
for file in Cargo.toml Cargo.lock LICENSE README.md THIRD_PARTY_NOTICES.md CHANGELOG.md; do
    grep -Fqx "aigate-cli-$version/$file" <<< "$members" || die "source archive excludes required file: $file"
done
(
    cd dist
    sha256sum "$archive" > SHA256SUMS
)
{
    printf '# AIGate CLI %s\n\n' "$tag"
    printf '%s\n\n' "$notes"
    if [[ $version == *-beta.* ]]; then
        printf 'Beta 预发行版：提供 Linux x86_64 / aarch64、Windows x64、macOS Apple Silicon 二进制与源码包。\n'
        printf 'Linux 基线为 Ubuntu 24.04 / glibc 2.39，需要 D-Bus 运行时库；macOS 基线为 15。二进制未签名或公证，桌面凭证库及真实 API 仍需实机验收。\n'
    else
        printf '本次提供源码包，安装步骤见 README。尚未提供预编译二进制；平台验收状态见 docs/platforms.md。\n'
    fi
} > dist/RELEASE_NOTES.md
printf 'Prepared source release in dist/ for %s (%s)\n' "$tag" "$commit"

#!/usr/bin/env bash
set -euo pipefail

die() { printf 'Nightly publication failed: %s\n' "$*" >&2; exit 1; }
[[ -n ${GH_REPO:-} ]] || die 'GH_REPO must identify the repository'
[[ ${NIGHTLY_ROLLING_ENABLED:-} == true ]] ||
    die 'set NIGHTLY_ROLLING_ENABLED=true only after a maintainer disables repository immutable releases'
scratch=$(mktemp -d)
staging_tag=
preserve_draft=false
cleanup() {
    status=$?
    if [[ -n $staging_tag ]]; then
        if "$preserve_draft"; then
            printf 'Publication interrupted; inspect draft %s and rerun Nightly to recover.\n' "$staging_tag" >&2
        else
            gh release delete "$staging_tag" --yes >/dev/null 2>&1 || true
        fi
    fi
    rm -rf -- "$scratch"
    exit "$status"
}
trap cleanup EXIT

# A missing resource is expected for a first release or tag.
# Authentication, permissions, rate limits and server failures must stop the run.
optional_api() {
    local response
    if response=$(gh api "$1" 2> "$scratch/api-error"); then
        printf '%s\n' "$response"
    elif grep -Fq '(HTTP 404)' "$scratch/api-error"; then
        printf 'null\n'
    else
        cat "$scratch/api-error" >&2
        return 1
    fi
}
# The repository settings endpoint requires Administration: read, unavailable
# to GITHUB_TOKEN. Use the maintainer opt-in above and inspect each release using
# Contents: read instead of requiring a long-lived administrator token in CI.
release=$(optional_api "repos/$GH_REPO/releases/tags/nightly")
if [[ $release != null ]]; then
    jq -e '.prerelease == true and .immutable != true' <<< "$release" >/dev/null ||
        die 'existing nightly must be a mutable prerelease; refusing to replace it'
fi
if [[ ${1:-} == --check && $# == 1 ]]; then
    printf 'Rolling nightly opt-in and existing release validated\n'
    exit 0
fi

[[ $# == 2 ]] || die 'usage: bash scripts/publish-nightly.sh [--check | SHA DIST]'
sha=$1
[[ $sha =~ ^[0-9a-f]{40}$ ]] || die 'expected a full commit SHA'
[[ -n ${GITHUB_RUN_ID:-} && -n ${GITHUB_RUN_ATTEMPT:-} ]] || die 'GitHub run identity is required'
cd -- "$2"
assets=(
    aigate-cli-nightly-x86_64-unknown-linux-gnu.tar.gz
    aigate-cli-nightly-aarch64-unknown-linux-gnu.tar.gz
    aigate-cli-nightly-x86_64-pc-windows-msvc.zip
    aigate-cli-nightly-aarch64-apple-darwin.tar.gz
    aigate-cli-nightly-source.tar.gz
)
for file in "${assets[@]}" SHA256SUMS RELEASE_NOTES.md; do
    [[ -s $file ]] || die "missing release output: $file"
done
[[ $(wc -l < SHA256SUMS) == "${#assets[@]}" ]] || die 'expected exactly five archive checksums'
for file in "${assets[@]}"; do
    grep -Eq "^[0-9a-f]{64}  $file$" SHA256SUMS || die "missing checksum for $file"
done
sha256sum --strict --check SHA256SUMS
grep -Fqx "Commit: $sha" RELEASE_NOTES.md || die 'release notes do not match the build commit'

# Upload and download-verify a private draft before touching the previous nightly.
staging_tag="nightly-staging-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"
gh release create "$staging_tag" "${assets[@]}" SHA256SUMS \
    --target "$sha" --draft --prerelease --latest=false \
    --title 'AIGate CLI nightly' --notes-file RELEASE_NOTES.md
gh release download "$staging_tag" --dir "$scratch/download"
cmp SHA256SUMS "$scratch/download/SHA256SUMS"
(
    cd "$scratch/download"
    sha256sum --strict --check SHA256SUMS
)

# GitHub cannot atomically replace a release. Keep the fully uploaded draft if
# promotion fails; fixed download URLs can be briefly unavailable at this point.
preserve_draft=true
if [[ $release != null ]]; then
    gh release delete nightly --yes
fi
ref=$(optional_api "repos/$GH_REPO/git/ref/tags/nightly")
if [[ $ref == null ]]; then
    gh api --method POST "repos/$GH_REPO/git/refs" -f ref=refs/tags/nightly -f sha="$sha" >/dev/null
else
    gh api --method PATCH "repos/$GH_REPO/git/refs/tags/nightly" -f sha="$sha" -F force=true >/dev/null
fi
gh release edit "$staging_tag" --tag nightly --verify-tag --draft=false --prerelease --latest=false \
    --title 'AIGate CLI nightly' --notes-file RELEASE_NOTES.md
staging_tag=
printf 'Published nightly at %s\n' "$sha"

#!/usr/bin/env bash
set -euo pipefail

project=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
fixture="$scratch/source repository"
mkdir -p "$fixture/scripts" "$fixture/src"
cp "$project/scripts/prepare-release.sh" "$fixture/scripts/"
cat > "$fixture/Cargo.toml" <<'EOF'
[package]
name = "aigate-cli"
version = "0.1.0"
edition = "2021"
publish = false
EOF
printf 'fn main() {}\n' > "$fixture/src/main.rs"
printf '/target/\n/dist/\n' > "$fixture/.gitignore"
printf '# Changes\n\n## [Unreleased]\n\n## [0.1.0]\n\n- Initial source release.\n' > "$fixture/CHANGELOG.md"
for file in LICENSE README.md THIRD_PARTY_NOTICES.md; do
    printf 'Release fixture: %s\n' "$file" > "$fixture/$file"
done

cd "$fixture"
# All commits and tags belong to this disposable fixture, never the project.
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR
git init --initial-branch=main --quiet
git config user.name 'Release Test'
git config user.email 'release-test@example.invalid'
git config commit.gpgsign false
git config tag.gpgsign false
git config core.hooksPath /dev/null
cargo generate-lockfile --offline --quiet
git add .
git commit --quiet -m 'test(release): create source fixture'
git tag v0.1.0

prepare() { bash scripts/prepare-release.sh "$@" > "$scratch/output" 2>&1; }
reject() {
    if prepare "$@"; then
        printf 'Expected rejection: %s\n' "$*" >&2
        exit 1
    fi
}

prepare --check v0.1.0
test ! -d dist
for tag in v0.2.0 v01.1.0 ../main 'v0.1.0;false'; do
    reject --check "$tag"
done
git tag v0.2.0
reject --check v0.2.0
printf '\nchanged\n' >> README.md
reject --check v0.1.0
git restore README.md
printf 'untracked\n' > untracked.txt
reject --check v0.1.0
rm untracked.txt

prepare v0.1.0
(
    cd dist
    sha256sum --check SHA256SUMS
)
tar tzf dist/aigate-cli-0.1.0-source.tar.gz > "$scratch/archive-files"
for file in Cargo.lock LICENSE README.md THIRD_PARTY_NOTICES.md; do
    grep -Fqx "aigate-cli-0.1.0/$file" "$scratch/archive-files"
done
if grep -Eq '/(dist|target|\.git)/' "$scratch/archive-files"; then
    printf 'Unexpected local files in source archive\n' >&2
    exit 1
fi
grep -q 'Initial source release' dist/RELEASE_NOTES.md
reject v0.1.0
cp dist/aigate-cli-0.1.0-source.tar.gz "$scratch/first.tar.gz"
rm -rf dist
prepare v0.1.0
cmp "$scratch/first.tar.gz" dist/aigate-cli-0.1.0-source.tar.gz

# Git attributes must not silently remove licensing from the archive.
printf 'LICENSE export-ignore\n' > .gitattributes
git add .gitattributes
git commit --quiet -m 'test(release): exclude required license'
git tag --force v0.1.0 >/dev/null
rm -rf dist
reject v0.1.0
grep -q 'excludes required file: LICENSE' "$scratch/output"
git rm --quiet .gitattributes
git commit --quiet -m 'test(release): restore source licensing'
git tag --force v0.1.0 >/dev/null

# A tag pointing at another commit cannot package the current checkout.
printf 'new revision\n' >> README.md
git add README.md
git commit --quiet -m 'test(release): advance fixture'
reject --check v0.1.0
git tag --force v0.1.0 >/dev/null
printf '# Changes\n\n## [Unreleased]\n\n- Pending work.\n' > CHANGELOG.md
git add CHANGELOG.md
git commit --quiet -m 'test(release): remove finalized notes'
git tag --force v0.1.0 >/dev/null
reject --check v0.1.0

# Beta releases must match the package version and have finalized notes too.
sed -i 's/version = "0.1.0"/version = "0.1.0-beta.1"/' Cargo.toml
printf '# Changes\n\n## [Unreleased]\n\n## [0.1.0-beta.1]\n\n- Initial beta release.\n' > CHANGELOG.md
cargo generate-lockfile --offline --quiet
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit --quiet -m 'test(release): prepare beta fixture'
git tag v0.1.0-beta.1
prepare --check v0.1.0-beta.1
for tag in v0.1.0-beta v0.1.0-beta.01 v0.1.0-rc.1 v0.1.0-beta.2; do
    git tag "$tag"
    reject --check "$tag"
done
rm -rf dist
prepare v0.1.0-beta.1
(cd dist && sha256sum --check SHA256SUMS)
tar -tzf dist/aigate-cli-0.1.0-beta.1-source.tar.gz > "$scratch/archive-files"
grep -Fqx 'aigate-cli-0.1.0-beta.1/Cargo.lock' "$scratch/archive-files"
grep -Fq 'Initial beta release' dist/RELEASE_NOTES.md
grep -Fq 'Linux x86_64 / aarch64' dist/RELEASE_NOTES.md

# Even a clean, tagged checkout must belong to main.
git checkout --quiet --orphan unrelated
git commit --quiet -m 'test(release): create unrelated history'
git tag --force v0.1.0 >/dev/null
reject --check v0.1.0
printf 'Release preparation tests passed\n'

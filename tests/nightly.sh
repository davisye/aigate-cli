#!/usr/bin/env bash
set -euo pipefail

project=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
mkdir -p "$scratch/bin" "$scratch/pristine"
export MOCK_STATE="$scratch/state"
export GH_REPO=example/aigate-cli GITHUB_RUN_ID=123 GITHUB_RUN_ATTEMPT=1
export NIGHTLY_ROLLING_ENABLED=true
sha=0123456789abcdef0123456789abcdef01234567
assets=(
    aigate-cli-nightly-x86_64-unknown-linux-gnu.tar.gz
    aigate-cli-nightly-aarch64-unknown-linux-gnu.tar.gz
    aigate-cli-nightly-x86_64-pc-windows-msvc.zip
    aigate-cli-nightly-aarch64-apple-darwin.tar.gz
    aigate-cli-nightly-source.tar.gz
)
for file in "${assets[@]}"; do
    printf 'Fixture archive: %s\n' "$file" > "$scratch/pristine/$file"
done
(
    cd "$scratch/pristine"
    sha256sum "${assets[@]}" > SHA256SUMS
)
printf 'Commit: %s\n' "$sha" > "$scratch/pristine/RELEASE_NOTES.md"

# This mock is the only gh executable used; tests never contact GitHub.
cat > "$scratch/bin/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$MOCK_STATE/log"
if [[ $1 == api ]]; then
    if [[ $2 == --method ]]; then
        [[ ${MOCK_FAILURE:-} != ref ]] || exit 1
        printf '{}\n'
        exit 0
    fi
    [[ ${MOCK_FAILURE:-} != api ]] || { printf 'gh: denied (HTTP 403)\n' >&2; exit 1; }
    case "$2" in
        */releases/tags/nightly)
            case "$MOCK_EXISTING" in
                missing) printf 'gh: Not Found (HTTP 404)\n' >&2; exit 1 ;;
                immutable) printf '{"prerelease":true,"immutable":true}\n' ;;
                stable) printf '{"prerelease":false,"immutable":false}\n' ;;
                *) printf '{"prerelease":true,"immutable":false}\n' ;;
            esac
            ;;
        */git/ref/tags/nightly)
            if [[ $MOCK_EXISTING == missing ]]; then
                printf 'gh: Not Found (HTTP 404)\n' >&2
                exit 1
            fi
            printf '{"object":{"sha":"previous"}}\n'
            ;;
        *) exit 2 ;;
    esac
elif [[ $1 == release ]]; then
    case "$2" in
        create)
            [[ ${MOCK_FAILURE:-} != upload ]] || exit 1
            mkdir -p "$MOCK_STATE/remote"
            shift 3
            while [[ $# -gt 0 && $1 != --* ]]; do
                cp -- "$1" "$MOCK_STATE/remote/"
                shift
            done
            ;;
        download)
            [[ $4 == --dir ]]
            mkdir -p "$5"
            cp "$MOCK_STATE/remote/"* "$5/"
            if [[ ${MOCK_FAILURE:-} == corrupt ]]; then
                printf 'damaged\n' >> "$5/aigate-cli-nightly-source.tar.gz"
            fi
            ;;
        delete) ;;
        edit) [[ ${MOCK_FAILURE:-} != promote ]] || exit 1 ;;
        *) exit 2 ;;
    esac
else
    exit 2
fi
SH
chmod +x "$scratch/bin/gh"
export PATH="$scratch/bin:$PATH"

reset_fixture() {
    rm -rf -- "$MOCK_STATE" "$scratch/dist"
    mkdir -p "$MOCK_STATE"
    : > "$MOCK_STATE/log"
    cp -R "$scratch/pristine" "$scratch/dist"
    export MOCK_EXISTING=mutable MOCK_FAILURE='' NIGHTLY_ROLLING_ENABLED=true
}
publish() { bash "$project/scripts/publish-nightly.sh" "$sha" "$scratch/dist" > "$scratch/output" 2>&1; }
reject() {
    if publish; then
        printf 'Expected nightly rejection: %s\n' "$MOCK_FAILURE/$MOCK_EXISTING" >&2
        exit 1
    fi
}
untouched() {
    if grep -Eq 'release delete nightly |api --method (POST|PATCH)|release edit' "$MOCK_STATE/log"; then
        printf 'Previous nightly was modified after a failed validation\n' >&2
        cat "$MOCK_STATE/log" >&2
        exit 1
    fi
}

reset_fixture
bash "$project/scripts/publish-nightly.sh" --check > "$scratch/output"
untouched
publish
grep -Fq 'api --method PATCH repos/example/aigate-cli/git/refs/tags/nightly' "$MOCK_STATE/log"
grep -Fq 'release edit nightly-staging-123-1 --tag nightly --verify-tag --draft=false --prerelease --latest=false' "$MOCK_STATE/log"
download_line=$(sed -n '/release download/=' "$MOCK_STATE/log")
delete_line=$(sed -n '/release delete nightly --yes/=' "$MOCK_STATE/log")
test "$download_line" -lt "$delete_line"

reset_fixture
export MOCK_EXISTING=missing
publish
grep -Fq 'api --method POST repos/example/aigate-cli/git/refs -f ref=refs/tags/nightly' "$MOCK_STATE/log"
if grep -Fq 'release delete nightly --yes' "$MOCK_STATE/log"; then
    printf 'First publication unexpectedly deleted nightly\n' >&2
    exit 1
fi

for failure in api upload corrupt; do
    reset_fixture
    export MOCK_FAILURE="$failure"
    reject
    untouched
done
reset_fixture
export NIGHTLY_ROLLING_ENABLED=false
reject
test ! -s "$MOCK_STATE/log"
for existing in immutable stable; do
    reset_fixture
    export MOCK_EXISTING="$existing"
    reject
    untouched
done
reset_fixture
rm "$scratch/dist/${assets[0]}"
reject
untouched
reset_fixture
printf 'corrupt\n' >> "$scratch/dist/${assets[0]}"
reject
untouched
reset_fixture
printf 'Commit: another-commit\n' > "$scratch/dist/RELEASE_NOTES.md"
reject
untouched
for failure in ref promote; do
    reset_fixture
    export MOCK_FAILURE="$failure"
    reject
    grep -Fq 'inspect draft nightly-staging-123-1' "$scratch/output"
    if grep -Fq 'release delete nightly-staging-123-1' "$MOCK_STATE/log"; then
        printf 'Recovery draft was deleted\n' >&2
        exit 1
    fi
done

# Exercise Unix archive layout, executable mode, metadata and extracted smoke
# checks with a tiny binary fixture instead of a real build in tmpfs.
fixture="$scratch/package"
mkdir -p "$fixture/target/x86_64-unknown-linux-gnu/release" "$fixture/target/aarch64-unknown-linux-gnu/release"
cat > "$fixture/target/x86_64-unknown-linux-gnu/release/aigate-cli" <<'SH'
#!/usr/bin/env bash
case "$1" in
    --version) printf 'aigate-cli %s\n' "$TEST_BINARY_VERSION" ;;
    --help) printf 'Help\n' ;;
    regions) printf '[]\n' ;;
    *) exit 1 ;;
esac
SH
chmod +x "$fixture/target/x86_64-unknown-linux-gnu/release/aigate-cli"
cp "$fixture/target/x86_64-unknown-linux-gnu/release/aigate-cli" "$fixture/target/aarch64-unknown-linux-gnu/release/"
for file in LICENSE THIRD_PARTY_NOTICES.md README.md CHANGELOG.md; do
    printf 'Fixture: %s\n' "$file" > "$fixture/$file"
done
(
    cd "$fixture"
    unset CARGO_TARGET_DIR
    for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
        for label in nightly 0.1.0-beta.1 0.1.0-rc.1; do
            export TEST_BINARY_VERSION=${label/nightly/0.1.0-beta.1}
            bash "$project/scripts/package-binary.sh" "$target" "$TEST_BINARY_VERSION" "$sha" "$label" > "$scratch/output"
            mkdir -p "extracted/$target/$label"
            tar -xzf "dist/aigate-cli-$label-$target.tar.gz" -C "extracted/$target/$label"
            test -x "extracted/$target/$label/aigate-cli"
            test -s "extracted/$target/$label/LICENSE"
            test -s "extracted/$target/$label/THIRD_PARTY_NOTICES.md"
            grep -Fqx "Commit: $sha" "extracted/$target/$label/BUILD-INFO.txt"
            grep -Fqx "Target: $target" "extracted/$target/$label/BUILD-INFO.txt"
        done
    done
    for label in 0.1.0-rc 0.1.0-rc.01 0.1.0-alpha.1 01.1.0-rc.1; do
        export TEST_BINARY_VERSION=$label
        if bash "$project/scripts/package-binary.sh" x86_64-unknown-linux-gnu "$TEST_BINARY_VERSION" "$sha" "$label" > "$scratch/output" 2>&1; then
            printf 'Invalid binary archive label accepted: %s\n' "$label" >&2
            exit 1
        fi
    done
)
printf 'Nightly packaging and publication tests passed\n'

#!/usr/bin/env bash
set -euo pipefail

project=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
umask 077
scratch=$(mktemp -d)
master="$scratch/master"
verify_home="$scratch/verify"
mkdir -p "$master" "$verify_home" "$scratch/runner" "$scratch/release with spaces"
cleanup() {
    gpgconf --homedir "$master" --kill all >/dev/null 2>&1 || true
    gpgconf --homedir "$verify_home" --kill all >/dev/null 2>&1 || true
    rm -rf -- "$scratch"
}
trap cleanup EXIT

# Disposable fixture only. Production keys and the user's keyring are never used.
passphrase='disposable-release-signing-test-only'
printf '%s' "$passphrase" | gpg --homedir "$master" --batch --pinentry-mode loopback --passphrase-fd 0 \
    --quick-generate-key 'Release signing fixture <release-test@example.invalid>' ed25519 cert 1d > "$scratch/keygen.log" 2>&1
primary=$(gpg --homedir "$master" --batch --with-colons --list-secret-keys 2>/dev/null | awk -F: '$1=="fpr" {print $10;exit}')
printf '%s' "$passphrase" | gpg --homedir "$master" --batch --pinentry-mode loopback --passphrase-fd 0 \
    --quick-add-key "$primary" ed25519 sign 1d >> "$scratch/keygen.log" 2>&1
signing=$(gpg --homedir "$master" --batch --with-colons --with-subkey-fingerprint --list-secret-keys 2>/dev/null | awk -F: '$1=="ssb" {wanted=1;next} wanted && $1=="fpr" {print $10;exit}')
printf '%s' "$passphrase" | gpg --homedir "$master" --batch --pinentry-mode loopback --passphrase-fd 0 \
    --armor --output "$scratch/subkey.asc" --export-secret-subkeys "$signing!"
printf '%s' "$passphrase" | gpg --homedir "$master" --batch --pinentry-mode loopback --passphrase-fd 0 \
    --armor --output "$scratch/full-key.asc" --export-secret-keys "$primary"
export RELEASE_GPG_PRIVATE_KEY RELEASE_GPG_PASSPHRASE RELEASE_GPG_PRIMARY_FINGERPRINT RELEASE_GPG_SIGNING_FINGERPRINT RUNNER_TEMP
RELEASE_GPG_PRIVATE_KEY=$(<"$scratch/subkey.asc")
RELEASE_GPG_PASSPHRASE=$passphrase
RELEASE_GPG_PRIMARY_FINGERPRINT=$primary
RELEASE_GPG_SIGNING_FINGERPRINT=$signing
RUNNER_TEMP="$scratch/runner"
dist="$scratch/release with spaces"
printf '%s\n' 'Disposable archive payload' > "$dist/aigate-cli-0.1.0-alpha.1-source.tar.gz"
(cd "$dist" && sha256sum aigate-cli-0.1.0-alpha.1-source.tar.gz > SHA256SUMS)

sign() { bash "$project/scripts/sign-release.sh" "$dist" > "$scratch/output" 2>&1; }
reject() {
    if sign; then printf '%s\n' 'Expected signing rejection' >&2; exit 1; fi
    [[ ! -e "$dist/SHA256SUMS.asc" && ! -e "$dist/aigate-cli-release-key.asc" ]]
    [[ -z $(find "$RUNNER_TEMP" -mindepth 1 -print -quit) ]]
}

RELEASE_GPG_PRIVATE_KEY='' reject
RELEASE_GPG_PASSPHRASE='' reject
RELEASE_GPG_SIGNING_FINGERPRINT="$primary" reject
RELEASE_GPG_SIGNING_FINGERPRINT='0000000000000000000000000000000000000000' reject
RELEASE_GPG_PRIVATE_KEY="$(<"$scratch/full-key.asc")" reject
grep -Fq 'import only the dedicated signing secret subkey' "$scratch/output"
RELEASE_GPG_PASSPHRASE='incorrect-disposable-test-passphrase' reject
printf '%s\n' 'Changed payload' >> "$dist/aigate-cli-0.1.0-alpha.1-source.tar.gz"
reject
(cd "$dist" && sha256sum aigate-cli-0.1.0-alpha.1-source.tar.gz > SHA256SUMS)

sign
[[ -s "$dist/SHA256SUMS.asc" && -s "$dist/aigate-cli-release-key.asc" ]]
[[ -z $(find "$RUNNER_TEMP" -mindepth 1 -print -quit) ]]
gpg --homedir "$verify_home" --batch --import "$dist/aigate-cli-release-key.asc" > "$scratch/verify.log" 2>&1
verified=$(gpg --homedir "$verify_home" --batch --status-fd 1 --verify "$dist/SHA256SUMS.asc" "$dist/SHA256SUMS" 2>> "$scratch/verify.log")
awk -v signing="$signing" '$2=="VALIDSIG" && $3==signing {found=1} END {exit(!found)}' <<< "$verified"
original=$(sha256sum "$dist/SHA256SUMS.asc")
if sign; then printf '%s\n' 'Existing signature must not be overwritten' >&2; exit 1; fi
[[ $(sha256sum "$dist/SHA256SUMS.asc") == "$original" ]]
printf '%s\n' 'tampered manifest' >> "$dist/SHA256SUMS"
if gpg --homedir "$verify_home" --batch --verify "$dist/SHA256SUMS.asc" "$dist/SHA256SUMS" >> "$scratch/verify.log" 2>&1; then
    printf '%s\n' 'Tampered manifest must fail verification' >&2
    exit 1
fi
printf '%s\n' 'Release signing tests passed'

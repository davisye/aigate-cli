#!/usr/bin/env bash
# The calling release job supplies only its dedicated signing secret subkey.
set +x
set -euo pipefail
umask 077

die() { printf 'Release signing failed: %s\n' "$*" >&2; exit 1; }
[[ $# == 1 && -d $1 ]] || die 'usage: bash scripts/sign-release.sh DIST_DIRECTORY'
for tool in gpg gpgconf sha256sum awk; do
    command -v "$tool" >/dev/null || die "missing tool: $tool"
done
[[ -n ${RELEASE_GPG_PRIVATE_KEY:-} ]] || die 'RELEASE_GPG_PRIVATE_KEY is required'
[[ -n ${RELEASE_GPG_PASSPHRASE:-} ]] || die 'RELEASE_GPG_PASSPHRASE is required'
primary=${RELEASE_GPG_PRIMARY_FINGERPRINT:-}
signing=${RELEASE_GPG_SIGNING_FINGERPRINT:-}
primary=${primary^^}
signing=${signing^^}
[[ $primary =~ ^([A-F0-9]{40}|[A-F0-9]{64})$ ]] || die 'expected a full primary fingerprint'
[[ $signing =~ ^([A-F0-9]{40}|[A-F0-9]{64})$ && $signing != "$primary" ]] || die 'expected a separate full signing subkey fingerprint'

cd -- "$1"
[[ -s SHA256SUMS && ! -L SHA256SUMS ]] || die 'missing regular SHA256SUMS'
[[ ! -e SHA256SUMS.asc && ! -e aigate-cli-release-key.asc ]] || die 'signature outputs already exist'
sha256sum --strict --check SHA256SUMS

signing_private_key=$RELEASE_GPG_PRIVATE_KEY
signing_passphrase=$RELEASE_GPG_PASSPHRASE
unset RELEASE_GPG_PRIVATE_KEY RELEASE_GPG_PASSPHRASE
signing_home=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/aigate-release-gpg.XXXXXX")
completed=false
cleanup() {
    gpgconf --homedir "$signing_home" --kill all >/dev/null 2>&1 || true
    rm -rf -- "$signing_home"
    if ! "$completed"; then rm -f -- SHA256SUMS.asc aigate-cli-release-key.asc; fi
}
trap cleanup EXIT
export GNUPGHOME=$signing_home
printf '%s' "$signing_private_key" | gpg --batch --import
unset signing_private_key
gpg --batch --with-colons --with-subkey-fingerprint --list-secret-keys > "$signing_home/key-listing"
# Refuse a full primary private key, extra secret subkeys, or a mismatched key.
awk -F: -v primary="$primary" -v signing="$signing" '
    $1 == "sec" { roots++; kind="root"; if ($15 != "#") bad=1 }
    $1 == "ssb" { subs++; kind="sub"; if ($12 != "s" || $15 != "+") bad=1 }
    $1 == "fpr" && kind == "root" { if ($10 != primary) bad=1; kind="" }
    $1 == "fpr" && kind == "sub" { if ($10 != signing) bad=1; kind="" }
    END { exit(bad || roots != 1 || subs != 1) }
' "$signing_home/key-listing" || die 'import only the dedicated signing secret subkey; primary and signing fingerprints must match'

printf '%s:6:\n' "$primary" | gpg --batch --import-ownertrust
printf '%s' "$signing_passphrase" | gpg --batch --pinentry-mode loopback --passphrase-fd 0 \
    --local-user "$signing!" --armor --output SHA256SUMS.asc --detach-sign SHA256SUMS
unset signing_passphrase
verification=$(gpg --batch --status-fd 1 --verify SHA256SUMS.asc SHA256SUMS)
awk -v signing="$signing" '$2 == "VALIDSIG" && $3 == signing { found=1 } END { exit(!found) }' \
    <<< "$verification" || die 'signature verification did not match the dedicated signing subkey'
gpg --batch --armor --output aigate-cli-release-key.asc --export "$primary"
[[ -s aigate-cli-release-key.asc ]] || die 'public key export failed'
completed=true
printf 'Signed SHA256SUMS with subkey %s (primary %s)\n' "$signing" "$primary"

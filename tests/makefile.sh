#!/bin/sh
# Exercise installation without compiling or writing outside a temporary fixture.
set -eu

project_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/aigate-cli-make.XXXXXX")
trap 'rm -rf -- "$test_dir"' 0
trap 'exit 1' HUP INT TERM

mkdir -p "$test_dir/bin" "$test_dir/work tree"
export TEST_CARGO_HOME_LOG="$test_dir/cargo-home.log"
export TEST_EXECUTABLE_LOG="$test_dir/executable.log"
export TEST_SUDO_LOG="$test_dir/sudo.log"
export TEST_ORIGINAL_HOME="$test_dir/original user"

cat > "$test_dir/bin/cargo" <<'SH'
#!/bin/sh
set -eu
printf '%s\n' "${CARGO_HOME-UNSET}" > "$TEST_CARGO_HOME_LOG"
printf '%s\n' "$0" > "$TEST_EXECUTABLE_LOG"
case "$1" in
    build)
        [ "${FAIL_BUILD:-0}" = 0 ] || exit 1
        mkdir -p "$CARGO_TARGET_DIR/release"
        printf '#!/bin/sh\nexit 0\n' > "$CARGO_TARGET_DIR/release/aigate-cli"
        ;;
    clean) rm -rf -- "$CARGO_TARGET_DIR" ;;
    *) exit 2 ;;
esac
SH

cat > "$test_dir/bin/getent" <<'SH'
#!/bin/sh
[ "$1" = passwd ] && [ "$2" = original-user ] || exit 1
printf 'original-user:x:1000:1000::%s:/bin/sh\n' "$TEST_ORIGINAL_HOME"
SH

cat > "$test_dir/bin/sudo" <<'SH'
#!/bin/sh
set -eu
[ "$1" = -H ] && [ "$2" = -u ] && [ "$3" = original-user ] || exit 1
printf '%s\n' "$3" > "$TEST_SUDO_LOG"
shift 3
exec "$@"
SH
chmod +x "$test_dir/bin/cargo" "$test_dir/bin/getent" "$test_dir/bin/sudo"
mkdir -p "$TEST_ORIGINAL_HOME/.cargo/bin"
cp "$test_dir/bin/cargo" "$TEST_ORIGINAL_HOME/.cargo/bin/cargo"

run_make() {
    # Ignore the invoking make's overrides so default-path checks stay meaningful.
    if env -u CARGO_HOME -u CARGO_TARGET_DIR -u SUDO_USER -u SUDO_UID \
        -u MAKEFLAGS -u MFLAGS -u MAKEOVERRIDES -u AIGATE_TOKEN \
        PATH="$test_dir/bin:$PATH" FAIL_BUILD="${FAIL_BUILD:-0}" \
        make --no-print-directory -C "$test_dir/work tree" \
        -f "$project_dir/Makefile" INSTALL_UID=1000 CARGO=cargo "$@" \
        > "$test_dir/make.log" 2>&1; then
        return 0
    else
        cat "$test_dir/make.log" >&2
        return 1
    fi
}

# Conventional Cargo defaults and a user installation staged under DESTDIR.
run_make install DESTDIR="$test_dir/user-stage"
test -f "$test_dir/work tree/target/release/aigate-cli"
test "$(cat "$TEST_CARGO_HOME_LOG")" = UNSET
user_bin="$test_dir/user-stage$HOME/.local/bin/aigate-cli"
test -x "$user_bin"
test "$(stat -c '%a' "$user_bin")" = 755
cmp "$test_dir/work tree/target/release/aigate-cli" "$user_bin"

# Explicit cache, output and install paths support spaces.
custom_home="$test_dir/cargo home"
custom_target="$test_dir/custom target"
mkdir -p "$custom_home/bin"
cp "$test_dir/bin/cargo" "$custom_home/bin/cargo"
touch "$custom_home/keep"
run_make install CARGO_HOME="$custom_home" CARGO_TARGET_DIR="$custom_target" \
    PREFIX="/opt/aigate tools" DESTDIR="$test_dir/custom-stage"
test "$(cat "$TEST_CARGO_HOME_LOG")" = "$custom_home"
custom_bin="$test_dir/custom-stage/opt/aigate tools/bin/aigate-cli"
cmp "$custom_target/release/aigate-cli" "$custom_bin"
touch "$(dirname -- "$custom_bin")/keep"
run_make uninstall PREFIX="/opt/aigate tools" DESTDIR="$test_dir/custom-stage"
test ! -e "$custom_bin"
test -f "$(dirname -- "$custom_bin")/keep"

# A failed build cannot install an existing, stale executable.
if FAIL_BUILD=1 run_make install DESTDIR="$test_dir/failed-stage" 2>/dev/null; then
    printf '%s\n' 'Expected installation to fail with the build' >&2
    exit 1
fi
test ! -e "$test_dir/failed-stage"

# Cleaning only affects the selected build directory.
run_make clean CARGO_TARGET_DIR="$custom_target"
test ! -e "$custom_target"
test -f "$custom_home/keep"
test -x "$user_bin"

# Root defaults and sudo forwarding use only the mock commands and staged paths.
run_make install INSTALL_UID=0 SUDO_USER= DESTDIR="$test_dir/root-stage"
test -x "$test_dir/root-stage/usr/local/bin/aigate-cli"
run_make install INSTALL_UID=0 SUDO_USER=original-user DESTDIR="$test_dir/sudo-stage"
test "$(cat "$TEST_SUDO_LOG")" = original-user
test "$(cat "$TEST_CARGO_HOME_LOG")" = UNSET
test "$(cat "$TEST_EXECUTABLE_LOG")" = "$TEST_ORIGINAL_HOME/.cargo/bin/cargo"
test -x "$test_dir/sudo-stage/usr/local/bin/aigate-cli"
run_make install INSTALL_UID=0 SUDO_USER=original-user CARGO_HOME="$custom_home" \
    CARGO_TARGET_DIR="$custom_target" DESTDIR="$test_dir/sudo-custom-stage"
test "$(cat "$TEST_CARGO_HOME_LOG")" = "$custom_home"
test "$(cat "$TEST_EXECUTABLE_LOG")" = "$custom_home/bin/cargo"
cmp "$custom_target/release/aigate-cli" "$test_dir/sudo-custom-stage/usr/local/bin/aigate-cli"

printf '%s\n' 'Makefile install tests passed'

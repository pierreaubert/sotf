#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$SCRIPT_DIR/build-release-local.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/private/tmp}/sotf-release-test.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT

mkdir -p "$TEST_ROOT/home" "$TEST_ROOT/bin" "$TEST_ROOT/project/scripts"
: > "$TEST_ROOT/home/.sotf-release.conf"
HOME="$TEST_ROOT/home"
export HOME

# Source the orchestrator without running main so the checks exercise its real
# helpers while every external build command remains a local mock.
source "$SCRIPT"
set +e

PROJECT_ROOT="$TEST_ROOT/project"
DIST_DIR="$PROJECT_ROOT/dist"
VERSION=9.8.7
AGGREGATE_ROOT="$TEST_ROOT/aggregate"
mkdir -p "$DIST_DIR" "$AGGREGATE_ROOT/sotf" "$AGGREGATE_ROOT/sotf-daw"
for sibling in sotf-systemwide sotf-capture autoeq math-audio sofa-reader sotf-3rdparties; do
    mkdir -p "$AGGREGATE_ROOT/$sibling"
done
for repo in "$AGGREGATE_ROOT" "$AGGREGATE_ROOT"/*; do
    [ -d "$repo" ] || continue
    git -C "$repo" init -q
    printf 'workspace fixture\n' > "$repo/Cargo.toml"
    git -C "$repo" add Cargo.toml
    if [ "$repo" = "$AGGREGATE_ROOT" ]; then
        printf 'aggregate fixture\n' > "$repo/Justfile"
        git -C "$repo" add Justfile
    fi
    git -c user.name=Test -c user.email=test@example.invalid -C "$repo" commit -qm fixture
done
mkdir -p "$AGGREGATE_ROOT/sotf/tests/data" "$AGGREGATE_ROOT/autoeq/data"
printf 'tracked fixture data\n' > "$AGGREGATE_ROOT/sotf/tests/data/fixture.bin"
printf 'untracked dataset\n' > "$AGGREGATE_ROOT/autoeq/data/local-dataset.bin"
git -C "$AGGREGATE_ROOT/sotf" add tests/data/fixture.bin
git -c user.name=Test -c user.email=test@example.invalid -C "$AGGREGATE_ROOT/sotf" commit -qm 'add tracked data fixture'
export PROJECT_ROOT VERSION TEST_ROOT

cat > "$TEST_ROOT/bin/rustup" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
cat > "$TEST_ROOT/bin/cargo" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$PROJECT_ROOT/cargo-commands.log"
if [ "${MOCK_CARGO_FAIL:-false}" = true ]; then
    exit 23
fi
if [[ "$*" == *"-p sotf-host"* && "${MOCK_HELPER_FAIL:-false}" = true ]]; then
    exit 24
fi
target=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = --target ]; then
        target="$2"
        shift 2
    else
        shift
    fi
done
mkdir -p "$PROJECT_ROOT/target/$target/dist"
printf 'fresh tui %s\n' "$target" > "$PROJECT_ROOT/target/$target/dist/sotf-tui"
printf 'fresh desktop %s\n' "$target" > "$PROJECT_ROOT/target/$target/dist/sotf-desktop"
printf 'fresh worker %s\n' "$target" > "$PROJECT_ROOT/target/$target/dist/sotf-external-plugin-worker"
printf 'fresh helper %s\n' "$target" > "$PROJECT_ROOT/target/$target/dist/sotf-macos-sandbox-helper"
MOCK
cat > "$TEST_ROOT/bin/lipo" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
cat > "$TEST_ROOT/bin/docker" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
cat > "$TEST_ROOT/bin/just" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$TEST_ROOT/just-commands.log"
case "$*" in
    *docker-linux-arm64*)
        mkdir -p dist
        printf 'arm tar\n' > "dist/sotf-desktop-${VERSION}-linux-arm64.tar.gz"
        printf 'arm image\n' > "dist/sotf-desktop-${VERSION}-linux-arm64.AppImage"
        ;;
    *)
        echo "unexpected mocked just command: $*" >&2
        exit 95
        ;;
esac
MOCK
chmod +x "$TEST_ROOT/bin/rustup" "$TEST_ROOT/bin/cargo" "$TEST_ROOT/bin/lipo" "$TEST_ROOT/bin/docker" "$TEST_ROOT/bin/just"
PATH="$TEST_ROOT/bin:$PATH"
export PATH

cat > "$PROJECT_ROOT/scripts/build-dmg-sotf.sh" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
arch=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = --arch ]; then
        arch="$2"
        shift 2
    else
        shift
    fi
done
mkdir -p "$PROJECT_ROOT/dist"
printf 'fresh image\n' > "$PROJECT_ROOT/dist/sotf-desktop-${VERSION}-macos-${arch}.dmg"
MOCK
chmod +x "$PROJECT_ROOT/scripts/build-dmg-sotf.sh"

cd "$PROJECT_ROOT"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
if ! build_macos_target arm64 aarch64-apple-darwin hal; then
    echo "expected mocked macOS target build to pass" >&2
    exit 1
fi
grep -q -- '-p sotf-gpui --features shipping,hal' "$PROJECT_ROOT/cargo-commands.log" || {
    echo "macOS desktop build omitted shipping features" >&2
    exit 1
}
grep -q -- '-p sotf-host --features external-plugin-au --bin sotf-external-plugin-worker --bin sotf-macos-sandbox-helper' "$PROJECT_ROOT/cargo-commands.log" || {
    echo "macOS build omitted the AU process helpers" >&2
    exit 1
}
for artifact in \
    "$DIST_DIR/sotf-tui-${VERSION}-macos-arm64" \
    "$DIST_DIR/sotf-desktop-${VERSION}-macos-arm64" \
    "$DIST_DIR/sotf-desktop-${VERSION}-macos-arm64.dmg"; do
    [ -s "$artifact" ] || { echo "missing fresh artifact: $artifact" >&2; exit 1; }
done

# A failed cargo build must not stage a stale target binary into dist.
rm -rf "$DIST_DIR" "$PROJECT_ROOT/target"
mkdir -p "$DIST_DIR" "$PROJECT_ROOT/target/aarch64-apple-darwin/dist"
printf 'stale\n' > "$DIST_DIR/sotf-tui-${VERSION}-macos-arm64"
printf 'stale\n' > "$PROJECT_ROOT/target/aarch64-apple-darwin/dist/sotf-tui"
clear_versioned_artifacts "$DIST_DIR"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
if MOCK_CARGO_FAIL=true build_macos_target arm64 aarch64-apple-darwin hal; then
    echo "expected mocked cargo failure" >&2
    exit 1
fi
[ ! -e "$DIST_DIR/sotf-tui-${VERSION}-macos-arm64" ] || {
    echo "stale artifact survived the fresh-build cleanup" >&2
    exit 1
}
[ "$BUILD_FAILED" = true ] || { echo "cargo failure was not aggregated" >&2; exit 1; }

BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
if MOCK_HELPER_FAIL=true build_macos_target arm64 aarch64-apple-darwin hal; then
    echo "expected missing AU helper build to fail the macOS target" >&2
    exit 1
fi
[ "$BUILD_FAILED" = true ] || { echo "helper failure was not aggregated" >&2; exit 1; }
[ ! -e "$DIST_DIR/sotf-desktop-${VERSION}-macos-arm64.dmg" ] || {
    echo "helper build failure produced an apparent desktop release" >&2
    exit 1
}

# A copy failure and a signing failure must also prevent an apparent success.
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
cp() { return 44; }
if build_macos_target arm64 aarch64-apple-darwin hal; then
    echo "expected mocked artifact copy failure" >&2
    exit 1
fi
unset -f cp
[ "$BUILD_FAILED" = true ] || { echo "copy failure was not aggregated" >&2; exit 1; }

cat > "$PROJECT_ROOT/scripts/sign-macos.sh" <<'MOCK'
#!/usr/bin/env bash
exit 45
MOCK
chmod +x "$PROJECT_ROOT/scripts/sign-macos.sh"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
SKIP_MACOS=false
SKIP_SIGN=false
DEVELOPER_ID=mock-identity
APPLE_ID=""
if build_macos; then
    echo "expected mocked macOS signing failure" >&2
    exit 1
fi
[[ "$BUILD_FAILED" = true ]] || { echo "signing failure was not aggregated" >&2; exit 1; }
unset DEVELOPER_ID APPLE_ID

# --arch arm64 builds only the selected macOS target while keeping its fresh
# binaries and DMG in the result set.
rm -rf "$DIST_DIR" "$PROJECT_ROOT/target"
mkdir -p "$DIST_DIR"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
SKIP_SIGN=true
ARCH_SELECTION=arm64
if ! build_macos; then
    echo "expected selected-architecture macOS build to pass" >&2
    exit 1
fi
[ -s "$DIST_DIR/sotf-desktop-${VERSION}-macos-arm64.dmg" ] || {
    echo "selected macOS ARM64 DMG missing" >&2
    exit 1
}
[ ! -e "$DIST_DIR/sotf-desktop-${VERSION}-macos-x86_64.dmg" ] || {
    echo "unselected macOS x86_64 DMG was built" >&2
    exit 1
}
ARCH_SELECTION=all
SKIP_SIGN=false

# The source manifest must cover sibling source paths and detect changed bytes.
# A submodule gitlink in the aggregate root must be skipped, not hashed: it
# has no file content in the parent index.
mkdir -p "$AGGREGATE_ROOT/skills/ms-rust-skill"
git -C "$AGGREGATE_ROOT" update-index --add --cacheinfo "160000,$(git -C "$AGGREGATE_ROOT" rev-parse HEAD),skills/ms-rust-skill"
manifest="$TEST_ROOT/source-manifest.sha256"
source_files="$TEST_ROOT/source-files.nul"
write_source_manifest "$AGGREGATE_ROOT" "$manifest" "$source_files" .test-manifest.sha256
grep -q '^# source sotf-daw head ' "$manifest"
grep -q 'sotf-daw/Cargo.toml' "$manifest"
grep -q 'sotf/tests/data/fixture.bin' "$manifest"
if grep -q 'autoeq/data/local-dataset.bin' "$manifest"; then
    echo "untracked local dataset entered Linux source manifest" >&2
    exit 1
fi
if grep -q 'skills/ms-rust-skill' "$manifest"; then
    echo "submodule gitlink entered the source manifest" >&2
    exit 1
fi
(cd "$AGGREGATE_ROOT" && grep -v '^#' "$manifest" | shasum -a 256 -c - >/dev/null)

# Exercise the Linux orchestration without a network connection. In the
# two-machine topology Linux ARM64 builds in local Docker (mocked docker +
# just above, snapshot materialized under dist/) and Linux x86_64 builds in
# Docker on the spin builder (mocked ssh/rsync below). The mocks materialize
# each snapshot under the test directory and perform the same hash checks.
LINUX_HOST=mock-builder
LINUX_PORT=22
LINUX_SSH_KEY=""
LINUX_PROJECT_DIR="$TEST_ROOT/remote/sotf"
MOCK_LINUX_COMMANDS=""
MOCK_LAST_SPIN_SNAPSHOT=""
mkdir -p "$TEST_ROOT/remote"
remote_sotf() {
    if [ -n "$MOCK_LAST_SPIN_SNAPSHOT" ]; then
        printf '%s/sotf\n' "$MOCK_LAST_SPIN_SNAPSHOT"
        return 0
    fi
    local snapshot result=""
    for snapshot in "$TEST_ROOT"/remote/.sotf-release-*; do
        [ -d "$snapshot" ] && result="$snapshot/sotf"
    done
    [ -n "$result" ] || return 1
    printf '%s\n' "$result"
}
linux_ssh_cmd() {
    local cmd="$4"
    local remote_root manifest dir
    case "$cmd" in
        "uname -m") printf 'x86_64\n' ;;
        *"just docker-linux-x86"*)
            dir="$(remote_sotf)"
            mkdir -p "$dir/dist"
            MOCK_LINUX_COMMANDS+=" spin-linux-x86"
            printf 'x86 tar\n' > "$dir/dist/sotf-desktop-${VERSION}-linux-x86_64.tar.gz"
            printf 'x86 image\n' > "$dir/dist/sotf-desktop-${VERSION}-linux-x86_64.AppImage"
            ;;
        *"just docker-windows-x86"*)
            dir="$(remote_sotf)"
            mkdir -p "$dir/dist"
            MOCK_LINUX_COMMANDS+=" spin-windows-x86"
            printf 'x86 exe\n' > "$dir/dist/sotf-tui-${VERSION}-windows-x86_64.exe"
            printf 'x86 msix\n' > "$dir/dist/sotf-desktop-${VERSION}-windows-x86_64.msix"
            ;;
        *"rm -f dist/*"*)
            dir="$(remote_sotf)"
            rm -f "$dir"/dist/*"$VERSION"*
            ;;
        mkdir*)
            dir="$(printf '%s\n' "$cmd" | sed -n "s/.*mkdir '\\([^']*\\)'$/\\1/p")"
            mkdir -p "$dir"
            MOCK_LAST_SPIN_SNAPSHOT="$dir"
            ;;
        *"shasum -a 256 -c"*)
            remote_root="$(printf '%s\n' "$cmd" | sed -n "s/^cd '\\([^']*\\)'.*/\\1/p")"
            manifest="$(printf '%s\n' "$cmd" | sed -n "s/.*grep -v '\\^#' '\\([^']*\\)'.*/\\1/p")"
            (cd "$remote_root" && grep -v '^#' "$manifest" | shasum -a 256 -c - >/dev/null)
            ;;
        *)
            echo "unexpected mocked ssh command: $cmd" >&2
            return 97
            ;;
    esac
}
rsync() {
    local last="" arg destination files_from=""
    for arg in "$@"; do
        last="$arg"
        case "$arg" in --files-from=*) files_from="${arg#--files-from=}" ;; esac
    done
    destination="${last#*:}"
    destination="${destination%/}"
    [ -d "$AGGREGATE_ROOT/sotf-daw" ] || return 98
    [ -n "$files_from" ] || return 99
    mkdir -p "$destination"
    local rel
    while IFS= read -r -d '' rel; do
        mkdir -p "$destination/$(dirname "$rel")"
        cp -P "$AGGREGATE_ROOT/$rel" "$destination/$rel"
    done < "$files_from"
}
rsync_from() {
    local remote_sotf_dir
    remote_sotf_dir="$(remote_sotf)"
    cp "$remote_sotf_dir/dist/"*"$VERSION"* "$DIST_DIR/"
}
file() {
    case "$*" in
        *linux-x86_64*) printf 'ELF 64-bit LSB pie executable, x86-64\n' ;;
        *linux-arm64*) printf 'ELF 64-bit LSB pie executable, ARM aarch64\n' ;;
        *windows-x86_64*) printf 'PE32+ executable (console) x86-64, for MS Windows\n' ;;
        *windows-arm64*) printf 'PE32+ executable (console) Aarch64, for MS Windows\n' ;;
        *) return 1 ;;
    esac
}
: > "$TEST_ROOT/just-commands.log"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
SKIP_LINUX=false
DRY_RUN=false
build_linux
[ "$BUILD_FAILED" = false ] || { echo "mocked Linux all-arch run was marked failed" >&2; exit 1; }
for artifact in \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-x86_64.tar.gz" \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-x86_64.AppImage" \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-arm64.tar.gz" \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-arm64.AppImage"; do
    [ -s "$artifact" ] || { echo "missing mocked Linux artifact: $artifact" >&2; exit 1; }
done
grep -q 'docker-linux-arm64' "$TEST_ROOT/just-commands.log" || {
    echo "Linux ARM64 did not use the local Docker recipe" >&2
    exit 1
}
[[ "$MOCK_LINUX_COMMANDS" == *"spin-linux-x86"* ]] || {
    echo "Linux x86_64 did not use Docker on spin" >&2
    exit 1
}
remote_root="$(dirname "$(remote_sotf)")"
remote_manifest="$remote_root/.release-source-manifest-${VERSION}-"*.sha256
[ -s $remote_manifest ] || {
    echo "Linux snapshot omitted its source manifest" >&2
    exit 1
}
grep -q 'sotf-daw/Cargo.toml' $remote_manifest
grep -q 'sotf/tests/data/fixture.bin' $remote_manifest
grep -q 'sotf-daw/Cargo.toml' "$DIST_DIR"/source-manifest-linux-"$VERSION"-*.sha256 || {
    echo "Linux snapshot manifest omitted sibling sources" >&2
    exit 1
}

printf 'changed source\n' >> "$AGGREGATE_ROOT/sotf-daw/Cargo.toml"
if (cd "$AGGREGATE_ROOT" && grep -v '^#' "$manifest" | shasum -a 256 -c - >/dev/null 2>&1); then
    echo "source hash manifest accepted modified sibling source" >&2
    exit 1
fi

# ARM selection builds Linux ARM64 locally only: no spin SSH at all.
rm -f "$DIST_DIR"/*"$VERSION"* "$DIST_DIR"/BUILD-SHA256SUMS-"$VERSION"
: > "$TEST_ROOT/just-commands.log"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
ARCH_SELECTION=arm64
MOCK_LINUX_COMMANDS=""
build_linux
[ "$BUILD_FAILED" = false ] || { echo "mocked Linux arm64 run was marked failed" >&2; exit 1; }
[ ! -e "$DIST_DIR/sotf-desktop-${VERSION}-linux-x86_64.tar.gz" ] || {
    echo "unselected Linux x86 target was built" >&2
    exit 1
}
[ -s "$DIST_DIR/sotf-desktop-${VERSION}-linux-arm64.AppImage" ] || {
    echo "selected Linux ARM target missing" >&2
    exit 1
}
[[ "$MOCK_LINUX_COMMANDS" != *spin* ]] || {
    echo "Linux ARM selection contacted the spin builder" >&2
    exit 1
}

# x86 selection builds Linux x86_64 on spin only: no local Docker recipe.
rm -f "$DIST_DIR"/*"$VERSION"* "$DIST_DIR"/BUILD-SHA256SUMS-"$VERSION"
: > "$TEST_ROOT/just-commands.log"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
ARCH_SELECTION=x86_64
MOCK_LINUX_COMMANDS=""
build_linux
[ "$BUILD_FAILED" = false ] || { echo "mocked Linux x86 run was marked failed" >&2; exit 1; }
[ -s "$DIST_DIR/sotf-desktop-${VERSION}-linux-x86_64.AppImage" ] || {
    echo "selected Linux x86 AppImage missing" >&2
    exit 1
}
[ ! -e "$DIST_DIR/sotf-desktop-${VERSION}-linux-arm64.AppImage" ] || {
    echo "unselected Linux ARM target was built" >&2
    exit 1
}
if grep -q 'docker-linux-arm64' "$TEST_ROOT/just-commands.log"; then
    echo "unselected Linux ARM Docker target was built locally" >&2
    exit 1
fi
ARCH_SELECTION=all

# Windows: ARM64 builds natively in the (mocked) UTM VM, x86_64 builds via
# Docker on spin. Each selection must build only (and exactly) its targets.
rm -f "$DIST_DIR"/*"$VERSION"* "$DIST_DIR"/BUILD-SHA256SUMS-"$VERSION" "$DIST_DIR"/source-manifest-*
mkdir -p "$TEST_ROOT/winremote"
WINDOWS_ARM_VM_NAME=mock-vm
WINDOWS_ARM_USER=pierre
WINDOWS_ARM_SSH_KEY=""
WINDOWS_ARM_PROJECT_DIR="$TEST_ROOT/winremote/sotf"
MOCK_WIN_COMMANDS=""
start_windows_arm_vm() {
    WINDOWS_ARM_HOST="pierre@mock-vm"
    MOCK_WIN_COMMANDS+=" utm-start"
}
detect_win_bash() { WIN_BASH_PATH=/mock/bash.exe; }
detect_win_rsync() { WIN_RSYNC_PATH=/mock/rsync; }
win_remote_sotf() {
    local snapshot result=""
    for snapshot in "$TEST_ROOT"/winremote/.sotf-release-*; do
        [ -d "$snapshot" ] && result="$snapshot/sotf"
    done
    [ -n "$result" ] || return 1
    printf '%s\n' "$result"
}
win_ssh_cmd() {
    local cmd="$4"
    local dir remote_root manifest
    case "$cmd" in
        *"sha256sum -c"*)
            remote_root="$(printf '%s\n' "$cmd" | sed -n "s/^cd '\([^']*\)'.*/\1/p")"
            manifest="$(printf '%s\n' "$cmd" | sed -n "s/.*grep -v '\^#' '\([^']*\)'.*/\1/p")"
            (cd "$remote_root" && grep -v '^#' "$manifest" | shasum -a 256 -c - >/dev/null)
            ;;
        *"build-msix.ps1"*"arm64"*)
            dir="$(win_remote_sotf)"
            mkdir -p "$dir/dist"
            printf 'arm64 msix\n' > "$dir/dist/sotf-desktop-${VERSION}-windows-arm64.msix"
            ;;
        *"cargo build"*)
            dir="$(win_remote_sotf)"
            mkdir -p "$dir/dist"
            MOCK_WIN_COMMANDS+=" arm64-build"
            printf 'arm64 tui exe\n' > "$dir/dist/sotf-tui-${VERSION}-windows-arm64.exe"
            ;;
        *"rm -f"*)
            dir="$(win_remote_sotf)"
            mkdir -p "$dir/dist"
            case "$cmd" in
                *".msix"*) rm -f "$dir"/dist/*"$VERSION"*.msix ;;
                *) rm -f "$dir"/dist/*"$VERSION"* ;;
            esac
            ;;
        *"mkdir"*)
            dir="$(printf '%s\n' "$cmd" | sed -n "s/.*mkdir '\([^']*\)'$/\1/p")"
            mkdir -p "$dir"
            ;;
        *)
            echo "unexpected mocked win_ssh command: $cmd" >&2
            return 96
            ;;
    esac
}
rsync_from_win() {
    local remote_win_dir
    remote_win_dir="$(win_remote_sotf)"
    cp "$remote_win_dir/dist/"*"$VERSION"* "$DIST_DIR/"
}
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
SKIP_WINDOWS=false
SKIP_SIGN=true
ARCH_SELECTION=arm64
build_windows
[ "$BUILD_FAILED" = false ] || { echo "mocked Windows arm64 run was marked failed" >&2; exit 1; }
for artifact in \
    "$DIST_DIR/sotf-tui-${VERSION}-windows-arm64.exe" \
    "$DIST_DIR/sotf-desktop-${VERSION}-windows-arm64.msix"; do
    [ -s "$artifact" ] || { echo "missing mocked Windows artifact: $artifact" >&2; exit 1; }
done
[ ! -e "$DIST_DIR/sotf-tui-${VERSION}-windows-x86_64.exe" ] || {
    echo "unselected Windows x86_64 target was built" >&2
    exit 1
}
[[ "$MOCK_WIN_COMMANDS" == *"utm-start"* && "$MOCK_WIN_COMMANDS" == *"arm64-build"* ]] || {
    echo "Windows arm64 did not build natively in the UTM VM" >&2
    exit 1
}
[[ "$MOCK_LINUX_COMMANDS" != *"spin-windows-x86"* ]] || {
    echo "unselected Windows x86_64 spin build ran" >&2
    exit 1
}
grep -q 'sotf-daw/Cargo.toml' "$DIST_DIR"/source-manifest-windows-arm64-"$VERSION"-*.sha256 || {
    echo "Windows snapshot manifest omitted sibling sources" >&2
    exit 1
}

# x86 selection builds Windows x86_64 on spin only: no UTM VM.
rm -f "$DIST_DIR"/*"$VERSION"* "$DIST_DIR"/BUILD-SHA256SUMS-"$VERSION" "$DIST_DIR"/source-manifest-*
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
ARCH_SELECTION=x86_64
MOCK_WIN_COMMANDS=""
MOCK_LINUX_COMMANDS=""
build_windows
[ "$BUILD_FAILED" = false ] || { echo "mocked Windows x86 run was marked failed" >&2; exit 1; }
for artifact in \
    "$DIST_DIR/sotf-tui-${VERSION}-windows-x86_64.exe" \
    "$DIST_DIR/sotf-desktop-${VERSION}-windows-x86_64.msix"; do
    [ -s "$artifact" ] || { echo "missing mocked Windows x86 artifact: $artifact" >&2; exit 1; }
done
[[ "$MOCK_LINUX_COMMANDS" == *"spin-windows-x86"* ]] || {
    echo "Windows x86_64 did not use Docker on spin" >&2
    exit 1
}
[[ "$MOCK_WIN_COMMANDS" != *"utm-start"* ]] || {
    echo "unselected Windows ARM VM was started" >&2
    exit 1
}
ARCH_SELECTION=all
SKIP_SIGN=false

# Skip-windows contributes no build result, while any recorded target failure
# makes main fail and prevents post-build signing/metadata.
BUILD_RESULTS=()
BUILD_FAILED=false
SKIP_WINDOWS=true
build_windows
[ "$BUILD_FAILED" = false ] || { echo "skipped Windows target marked the run failed" >&2; exit 1; }
preflight() { return 0; }
build_macos() { record_result 'macOS ARM64: FAILED (mock)'; return 1; }
build_linux() { return 0; }
post_build() { echo "post-build must not run after a required-target failure" >&2; return 90; }
print_summary() { :; }
DRY_RUN=false
POST_ONLY=false
SKIP_MACOS=false
SKIP_LINUX=false
SKIP_WINDOWS=true
if main; then
    echo "main reported success after a required-target failure" >&2
    exit 1
fi

echo "build-release-local mocked regressions passed"

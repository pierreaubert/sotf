#!/usr/bin/env bash
# Keep AU process isolation helpers beside the desktop executable.

build_macos_plugin_helpers() {
    local daw_root="$1"
    local target_dir="$2"
    local target="${3:-}"
    local -a arguments=(build --profile dist --manifest-path "$daw_root/Cargo.toml"
        --target-dir "$target_dir" -p sotf-host --features external-plugin-au
        --bin sotf-external-plugin-worker --bin sotf-macos-sandbox-helper)
    if [ -n "$target" ]; then
        arguments+=(--target "$target")
    fi
    MACOSX_DEPLOYMENT_TARGET=15.0 cargo "${arguments[@]}"
}

stage_macos_plugin_helpers() {
    local source_dir="$1"
    local destination="$2"
    local desktop="$3"
    local helper architecture
    local architectures
    architectures=$(lipo -archs "$desktop") || return 1
    # Validate the complete pair before copying either executable.
    for helper in sotf-external-plugin-worker sotf-macos-sandbox-helper; do
        if [ ! -f "$source_dir/$helper" ] || [ ! -x "$source_dir/$helper" ]; then
            printf 'ERROR: Required macOS plugin helper is missing or not executable: %s\n' \
                "$source_dir/$helper" >&2
            return 1
        fi
        for architecture in $architectures; do
            if ! lipo -verify_arch "$architecture" "$source_dir/$helper"; then
                printf 'ERROR: Plugin helper %s does not support desktop architecture %s\n' \
                    "$helper" "$architecture" >&2
                return 1
            fi
        done
    done
    mkdir -p "$destination" || return 1
    for helper in sotf-external-plugin-worker sotf-macos-sandbox-helper; do
        cp "$source_dir/$helper" "$destination/$helper" || return 1
        chmod 755 "$destination/$helper" || return 1
    done
}

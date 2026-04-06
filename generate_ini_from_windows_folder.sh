#!/usr/bin/env bash

set -eu

if [ $# -ne 2 ]; then
    echo "Usage: $0 <ini_basename> <windows_folder>"
    exit 1
fi

INI_BASENAME=$1
WINDOWS_FOLDER=$2

# Case-insensitive folder lookup
find_dir() {
    find "$1" -maxdepth 1 -iname "$2" -type d 2>/dev/null | head -1
}

# Find the latest WinSxS subdirectory matching a component pattern.
# Args: WINSXS_LISTING ARCH_PREFIX COMPONENT_PATTERN [VERSION_PREFIX]
# WinSxS dir format: {arch}_{component}_{hash}_{version}_{locale}_{hash2}
# Normalizes to lowercase and replaces '-' with '.' for OS-agnostic matching.
find_latest_winsxs() {
    local listing="$1" arch="$2" pattern="$3" vprefix="${4:-}"
    local best_version="" best_dir="" dir base version
    local filtered
    filtered=$(grep -i "/${arch}_[^/]*${pattern}" <<< "$listing") || true
    while IFS= read -r dir; do
        [ -z "$dir" ] && continue
        base="${dir##*/}"
        # Version is the 4th underscore-delimited field
        local rest="${base#*_}" # drop arch
        rest="${rest#*_}"       # drop component
        rest="${rest#*_}"       # drop hash
        version="${rest%%_*}"   # take version
        if [ -z "$vprefix" ] || [[ "$version" == "${vprefix}"* ]]; then
            if [ -z "$best_version" ] || [[ "$(printf '%s\n%s' "$best_version" "$version" | sort -V | tail -1)" == "$version" ]]; then
                best_version="$version"
                best_dir="$dir"
            fi
        fi
    done <<< "$filtered"
    printf '%s' "$best_dir"
}

# Find WinSxS directories for special SxS-only DLLs.
# Args: WINSXS_LISTING ARCH_PREFIX...
# Sets WINSXS_EXTRA array with the found directories.
collect_winsxs_extras() {
    local listing="$1"; shift
    WINSXS_EXTRA=()
    local dir arch

    local -a specs=(
        "windows.winhttp:5.1"
        "common.controls:6.0"
        "gdiplus:1.0"
    )

    for spec in "${specs[@]}"; do
        local pattern="${spec%%:*}" vprefix="${spec##*:}"
        dir=""
        for arch in "$@"; do
            dir=$(find_latest_winsxs "$listing" "$arch" "$pattern" "$vprefix")
            [ -n "$dir" ] && break
        done
        if [ -n "$dir" ]; then
            echo "WinSxS: $pattern $vprefix (${arch}) -> $(basename "$dir")"
            WINSXS_EXTRA+=("$dir")
        fi
    done

    # MSVC Runtime: try XP naming first, then Win7 naming
    dir=""
    for arch in "$@"; do
        dir=$(find_latest_winsxs "$listing" "$arch" "cplusplus.runtime")
        [ -n "$dir" ] && break
        dir=$(find_latest_winsxs "$listing" "$arch" "msvcrt")
        [ -n "$dir" ] && break
    done
    if [ -n "$dir" ]; then
        echo "WinSxS: msvcrt (${arch}) -> $(basename "$dir")"
        WINSXS_EXTRA+=("$dir")
    fi
}

SYSTEM32=$(find_dir "$WINDOWS_FOLDER" "system32")
if [ -z "$SYSTEM32" ]; then
    echo "No system32 folder in $WINDOWS_FOLDER"
    exit 1
fi

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

cargo build --release

OUT_DIR=premade_ini

RUN="target/release/windows_dll_compat_checker"
# Don't want to accidentally include my VM's Guest Additions
IGNORE="\
-i VBoxMRXNP.dll \
-i VBoxControl.exe \
-i VBoxDisp.dll \
-i VBoxHook.dll \
"

SYSWOW64=$(find_dir "$WINDOWS_FOLDER" "syswow64")

WINSXS=$(find_dir "$WINDOWS_FOLDER" "winsxs")
WINSXS_LISTING=""
if [ -n "$WINSXS" ]; then
    WINSXS_LISTING=$(find "$WINSXS" -maxdepth 1 -type d 2>/dev/null)
fi

ARCH=i686
if [ -n "$SYSWOW64" ]; then
    ARCH=x86_64
    if [ -n "$WINSXS_LISTING" ]; then
        collect_winsxs_extras "$WINSXS_LISTING" x86 wow64
    else
        WINSXS_EXTRA=()
    fi
    $RUN $IGNORE --export-ini "$OUT_DIR/${INI_BASENAME}_${ARCH}_32bit_dlls.ini" "$SYSWOW64" ${WINSXS_EXTRA[@]+"${WINSXS_EXTRA[@]}"}
fi

if [ -n "$WINSXS_LISTING" ]; then
    if [ "$ARCH" = "x86_64" ]; then
        collect_winsxs_extras "$WINSXS_LISTING" amd64
    else
        collect_winsxs_extras "$WINSXS_LISTING" x86
    fi
else
    WINSXS_EXTRA=()
fi
$RUN $IGNORE --export-ini "$OUT_DIR/${INI_BASENAME}_${ARCH}.ini" "$SYSTEM32" ${WINSXS_EXTRA[@]+"${WINSXS_EXTRA[@]}"}

if [ -n "$SYSWOW64" ]; then
    $RUN --in-place --merge-common "$OUT_DIR/${INI_BASENAME}_${ARCH}_common.ini" "$OUT_DIR/${INI_BASENAME}_${ARCH}.ini" "$OUT_DIR/${INI_BASENAME}_${ARCH}_32bit_dlls.ini"
fi

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

SYSTEM32=$(find_dir "$WINDOWS_FOLDER" "system32")
if [ -z "$SYSTEM32" ]; then
    echo "No system32 folder in $WINDOWS_FOLDER"
    exit 1
fi

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

OUT_DIR=premade_ini

RUN="cargo run --release --"

SYSWOW64=$(find_dir "$WINDOWS_FOLDER" "syswow64")

ARCH=i686
if [ -n "$SYSWOW64" ]; then
    ARCH=x86_64
    $RUN --export-ini "$OUT_DIR/${INI_BASENAME}_${ARCH}_32bit_dlls.ini" "$SYSWOW64"
fi

$RUN --export-ini "$OUT_DIR/${INI_BASENAME}_${ARCH}.ini" "$SYSTEM32"

if [ -n "$SYSWOW64" ]; then
    $RUN --in-place --merge-common "$OUT_DIR/${INI_BASENAME}_${ARCH}_common.ini" "$OUT_DIR/${INI_BASENAME}_${ARCH}.ini" "$OUT_DIR/${INI_BASENAME}_${ARCH}_32bit_dlls.ini"
fi

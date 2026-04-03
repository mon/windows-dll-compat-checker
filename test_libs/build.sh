#!/usr/bin/env bash

set -eux

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

source /opt/llvm-mingw/toolchain-files/make/i686-mingw32-clang

make

#!/usr/bin/env bash

set -eu

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

docker run -it --rm \
    -v "$SCRIPT_DIR:/work"\
    -w "/work"\
    montymintypie/llvm-mingw-xp:22 \
    ./build.sh

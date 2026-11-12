#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 2 ]]; then
    echo "Usage: bash ci/prepare-esp32c3-tools.sh ARCHIVES_DIR TOOLS_DIR" >&2
    exit 2
fi

mkdir -p "$1" "$2"
archives=$(cd "$1" && pwd)
tools=$(cd "$2" && pwd)

fetch() {
    local file="$archives/$1" digest=$2 url=$3
    if [[ ! -f "$file" ]]; then
        curl --fail --location --retry 3 -o "$file.part" "$url"
        mv "$file.part" "$file"
    fi
    printf '%s  %s\n' "$digest" "$file" | shasum -a 256 -c
}

fetch qemu.tar.xz afa59465495295b8aa4f804768983fa30b0bbf33d0f68a047f9caa436b9164c0 \
    https://github.com/espressif/qemu/releases/download/esp-develop-9.2.2-20261109/qemu-riscv32-softmmu-esp_develop_9.2.2_20261109-x86_64-linux-gnu.tar.xz
fetch espflash.zip ac2031bd1f04c9107d9ba0e977535daa885a9f533f937dc369debd77f83665cd \
    https://github.com/esp-rs/espflash/releases/download/v4.4.0/espflash-x86_64-unknown-linux-gnu.zip

tar -xf "$archives/qemu.tar.xz" -C "$tools"
unzip -oq "$archives/espflash.zip" -d "$tools"

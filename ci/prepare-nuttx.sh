#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 4 || ! "$3" =~ ^[0-9a-f]{40}$ || ! "$4" =~ ^[0-9a-f]{40}$ ]]; then
    echo "Usage: bash ci/prepare-nuttx.sh ARCHIVES_DIR BUILD_DIR NUTTX_SHA APPS_SHA" >&2
    exit 2
fi

mkdir -p "$1" "$2"
archives=$(cd "$1" && pwd)
build=$(cd "$2" && pwd)

prepare_tree() {
    local repo=$1 tree=$2 sha=$3 archive="$archives/$2-$3.tar.gz"
    if [[ ! -f "$archive" ]]; then
        echo "Download $repo at $sha"
        time curl --fail --location --retry 3 --connect-timeout 15 --max-time 180 \
            -o "$archive.part" "https://codeload.github.com/apache/$repo/tar.gz/$sha"
        mv "$archive.part" "$archive"
    fi
    mkdir "$build/$tree"
    echo "Extract $repo at $sha"
    time tar -xzf "$archive" --strip-components=1 -C "$build/$tree"
}

prepare_tree nuttx nuttx "$3"
prepare_tree nuttx-apps apps "$4"

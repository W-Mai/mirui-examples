#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 2 || ( "$2" != dev && "$2" != release ) ]]; then
    echo "Usage: bash ci/build-nuttx.sh NUTTX_DIR dev|release" >&2
    exit 2
fi

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
nuttx=$(cd "$1" && pwd)
apps=$(cd "$nuttx/../apps" && pwd)
demo="$repo/examples/nuttx_demo"
profile="$2"

if [[ -e "$nuttx/.config" ]]; then
    echo "Use a clean NuttX checkout for each profile" >&2
    exit 2
fi

mkdir "$apps/external"
ln -s "$demo" "$apps/external/nuttx_demo"
printf 'source "$APPSDIR/external/nuttx_demo/Kconfig"\n' > "$apps/external/Kconfig"
printf '%s\n' 'include $(APPDIR)/external/nuttx_demo/Make.defs' > "$apps/external/Make.defs"

make_command=make
host=-l
if [[ "$(uname -s)" == Darwin ]]; then
    make_command=gmake
    host=-m
fi

cd "$nuttx"
bash tools/configure.sh "$host" rv-virt:fb
kconfig-tweak --disable ARCH_FPU --disable ARCH_DPFPU \
    --disable LIBM_TOOLCHAIN --enable LIBM --enable FB_SYNC \
    --enable DEV_URANDOM --enable FS_LARGEFILE --set-val TLS_NELEM 16 \
    --enable INPUT --enable EXAMPLES_RUST_MIRUI_DEMO
if [[ "$profile" == release ]]; then
    kconfig-tweak --enable DEBUG_FULLOPT
    stack_size=32768
else
    kconfig-tweak --disable DEBUG_FULLOPT --enable DEBUG_NOOPT
    stack_size=65536
fi
"$make_command" olddefconfig

for option in CONFIG_VIDEO_FB=y CONFIG_DRIVERS_VIRTIO_GPU=y CONFIG_LIBM=y \
    CONFIG_FB_SYNC=y CONFIG_DEV_URANDOM=y CONFIG_FS_LARGEFILE=y CONFIG_TLS_NELEM=16 \
    CONFIG_EXAMPLES_RUST_MIRUI_DEMO=y "CONFIG_EXAMPLES_RUST_MIRUI_DEMO_STACKSIZE=$stack_size"; do
    if ! grep -Fxq "$option" .config; then
        echo "Missing NuttX setting: $option" >&2
        exit 1
    fi
done

# NuttX's Rust.mk selects the Cargo profile and link path but has no lock flag.
cargo_bin=$(mktemp -d)
trap 'rm -rf "$cargo_bin"' EXIT
export MIRUI_CARGO
MIRUI_CARGO=$(command -v cargo)
printf '%s\n' '#!/usr/bin/env bash' \
    'if [[ "${1:-}" == build ]]; then' \
    '    shift' \
    '    exec "$MIRUI_CARGO" build --locked "$@"' \
    'fi' \
    'exec "$MIRUI_CARGO" "$@"' > "$cargo_bin/cargo"
chmod +x "$cargo_bin/cargo"

PATH="$cargo_bin:$PATH" CARGO_TARGET_DIR="$demo/target" "$make_command" -j"${BUILD_JOBS:-4}" \
    RUST_TARGET_TRIPLE=riscv32imac-unknown-nuttx-elf

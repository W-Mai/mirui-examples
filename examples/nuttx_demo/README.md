# mirui-nuttx-demo

mirui Rust UI on Apache NuttX RTOS. Targets `/dev/fb0` framebuffer +
`/dev/inputN` touchscreen.

## Setup

NuttX kernel + apps tree:

```bash
git clone https://github.com/apache/nuttx.git
git clone https://github.com/apache/nuttx-apps.git apps
ln -s $PWD/mirui-examples/examples/nuttx_demo apps/external/nuttx_demo
```

Required NuttX kbuild flags (per the official Rust integration guide):

```
CONFIG_SYSTEM_TIME64=y
CONFIG_FS_LARGEFILE=y
CONFIG_TLS_NELEM=16
CONFIG_DEV_URANDOM=y
CONFIG_VIDEO_FB=y
CONFIG_INPUT=y
# Disable CONFIG_ARCH_FPU on rv-virt (riscv32imac doesn't pair with FPU).
```

Rust support is implicit — NuttX wires `apps/tools/Rust.mk` based on
`LLVM_ARCHTYPE` / `LLVM_ABITYPE`; there is no `CONFIG_RUST`.

The demo strict-mode requires these to be **off**:
- `CONFIG_FB_OVERLAY` — adds `noverlays` to `fb_videoinfo_s`
- `CONFIG_FB_MODULEINFO` — adds 128-byte `moduleinfo` field
- `CONFIG_FB_HWCURSOR` / `CONFIG_FB_CMAP` — extra struct mirrors not in v1

## Reference board: `rv-virt:fb` under qemu-system-riscv32

The demo opens `/dev/fb0`, so it needs the framebuffer board
(`rv-virt:fb`) and a qemu virtio-gpu device — the serial-only
`rv-virt:nsh` / `-nographic` path has no framebuffer and the demo would
fail at `NuttxFbSurface::open`.

```bash
cd nuttx
./tools/configure.sh rv-virt:fb
make menuconfig
# enable Application Configuration → Examples → mirui Rust UI demo
make

qemu-system-riscv32 -semihosting -M virt -cpu rv32 -smp 8 -bios none \
    -kernel ./nuttx \
    -device virtio-gpu-device,xres=640,yres=480,bus=virtio-mmio-bus.0 \
    -serial stdio
nsh> mirui
```

The demo defaults to a 32 KiB task stack with `CONFIG_DEBUG_FULLOPT=y` (Cargo release) and 64 KiB otherwise (Cargo debug). When switching an existing NuttX configuration to debug, check `CONFIG_EXAMPLES_RUST_MIRUI_DEMO_STACKSIZE`: a saved 32 KiB value overrides the new default and can overflow during the first layout. Set it to 65536 before rebuilding.

## Toolchain

NuttX requires nightly and the `rust-src` component. NuttX's `apps/tools/Rust.mk` supplies the `-Zbuild-std=std,panic_abort` flags. CI uses `nightly-2026-11-11` and builds both profiles for `riscv32imac-unknown-nuttx-elf`.

## Runtime checks

The [NuttX workflow](../../.github/workflows/nuttx-example.yml) builds and boots a fresh kernel with the example linked in each profile. The check starts `mirui` from NSH, verifies two distinct Widgets frames on the VirtIO display, and checks that the task is still running. The job records the serial log, QEMU log, and framebuffer captures.

To run the same check locally, use the NuttX and apps revisions listed in the workflow. Put the two clean checkouts next to one another with the apps checkout named `apps`. Run the following from the Examples checkout with the cross compiler, NuttX Kconfig tools, and QEMU on `PATH`:

```bash
export RUSTUP_TOOLCHAIN=nightly-2026-11-11
bash ci/build-nuttx.sh /path/to/nuttx dev   # or release with a separate clean tree
python3 ci/test-nuttx.py /path/to/nuttx/nuttx /path/to/disk.qcow2 nuttx-results
```

The test opens the supplied qcow2 disk in snapshot mode. It checks rendering and task liveness; touch, keyboard, and frame pacing require separate validation.

## Status

- Framebuffer: `/dev/fbN` via `FBIOGET_VIDEOINFO` / `FBIOGET_PLANEINFO`,
  zero-copy direct pointer (no mmap needed).
- Touch: `/dev/inputN` `touch_sample_s` reader, multi-touch slot tracking
  via the cross-platform `PointerState` shared with linux-fb.
- Keyboard: optional `/dev/kbdN` reader, X11 keysyms mapped to mirui
  keycodes.
- Signal: SIGTERM / SIGINT → `InputEvent::Quit`.
- Multi-display: `NuttxConfig::display_index` picks `/dev/fbN`.

## License

MIT — same as mirui itself.

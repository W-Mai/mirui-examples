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

## Toolchain

NuttX is rustc tier-3 — needs nightly + `rust-src` component +
`-Zbuild-std=std,panic_abort`. NuttX `apps/tools/Rust.mk` handles the
flags. Pin a known-green nightly in `rust-toolchain.toml` if the
default rolls forward and breaks `-Zbuild-std`.

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

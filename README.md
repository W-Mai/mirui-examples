# mirui-examples

Hardware examples for [mirui](https://github.com/W-Mai/mirui) — running on real MCUs.

## Hardware

- **MCU**: ESP32-C3 Super Mini
- **Display**: ST7735S 128×128 SPI TFT (MSP1443)
- **Wiring**:

| Display | GPIO |
|---------|------|
| SCL | 5 |
| SDA | 4 |
| CS | 6 |
| RS/DC | 7 |
| RST | 3 |
| LED | 2 |
| 3V3 | 3V3 |
| GND | GND |

## Prerequisites

- Rust nightly (edition 2024)
- Target: `rustup target add riscv32imc-unknown-none-elf`
- Flash tool: `cargo install espflash`
- PlatformIO (bootloader only): `pip install platformio`

## First Time: Flash Bootloader

ESP32-C3 Super Mini (chip rev v0.4) requires a compatible bootloader with `min_chip_rev=0`. **Flash once**:

```bash
cd bootloader
pio run -t upload
```

This installs an ESP-IDF bootloader that accepts any chip revision. All Rust examples will boot normally after this.

## Running an Example

```bash
cd examples/esp32c3-animation
cargo build --release
espflash flash target/riscv32imc-unknown-none-elf/release/mirui-esp32c3 \
  --port /dev/cu.usbmodem1101 \
  --bootloader ../../bootloader/.pio/build/esp32c3/bootloader.bin
```

> Serial port name may vary. Use `ls /dev/cu.usb*` to find yours.

## Local mirui Development

By default this repo pulls `mirui` and `mirui-macros` from their
published git source — anyone can `git clone` and `cargo build` without
a sibling mirui checkout. To work against an unpublished mirui
working tree (e.g. while iterating on framework code in `../miru`),
edit the workspace-level `.cargo/config.toml` and uncomment the
`paths` override at the bottom of the file. The exact paths depend on
your layout; the defaults assume mirui sits next to this repo.

After editing, tell git to ignore your local changes to that file so
they don't get committed by accident:

```bash
git update-index --skip-worktree .cargo/config.toml
```

To resume tracking edits (e.g. to commit a new shared alias or build
setting), reverse it:

```bash
git update-index --no-skip-worktree .cargo/config.toml
```

`cargo build` will warn that `paths` overrides are deprecated; that's
expected and harmless. The override only affects builds in this
repo's tree.

## Examples

| Directory | Description |
|-----------|-------------|
| `esp32c3-animation` | Three-body simulation with ECS absolute positioning, dirty-rect partial refresh (160fps), mirui DSL UI background |

## Performance

- **Full-screen refresh**: 60fps (SPI 26MHz bottleneck, 32KB/frame)
- **Partial refresh (dirty rect)**: 160fps (only changed regions transmitted)
- **Binary size**: ~25KB .text (mirui + app code)

## Known Issues

- esp-hal 1.1 requires a custom bootloader for ESP32-C3 rev v0.4 chips
- SPI stable at 26MHz; 40MHz causes artifacts over jumper wires

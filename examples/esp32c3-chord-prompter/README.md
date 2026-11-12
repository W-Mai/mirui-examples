# ESP32-C3 Chord Prompter

A BLE-controlled chord and lyric display with a device-owned musical clock, counted beats and a strumming cursor. Song sheets use a front matter block and scale-degree chord groups; the browser console sends UTF-8 source to the board for validation and atomic activation.

## Hardware

Use the ESP32-C3 Super Mini and ST7735S 128×128 display wiring from the repository README. This example reuses the animation example's board driver. No speaker or buzzer is required: the board provides visual beats. The regular animation firmware does not include this example's BLE or RTOS dependencies.

## Build and flash

The directory pins Rust 1.97.0 and the RV32IMC target. From this directory:

```sh
cargo build --release
espflash flash --monitor --port /dev/cu.usbmodem1101 target/riscv32imc-unknown-none-elf/release/mirui-esp32c3-chord-prompter
```

The port varies by host. The application uses a 200 KiB heap, a single BLE connection, a 64-byte ATT packet pool and an 8 KiB upload staging buffer. `cargo build --release --features heap-stats` adds allocator peak diagnostics to control receipts on the serial console; the default build omits those counters. Optimization level 3 and LTO prioritize rendering throughput; check the linked image against the board's flash partition.

The shared `chord_prompter` demo must be present in the MIRUI git revision being built. Local development can use the sibling checkout overrides documented in the repository README.

### Physical BLE check

Flash the release firmware on the test board. With `bleak` installed and Bluetooth available in the test environment, run these commands from the repository root:

```bash
python3 ci/test-esp32c3-device.py scan
python3 ci/test-esp32c3-device.py ble "$PROMPTER_ADDRESS"
```

Set `PROMPTER_ADDRESS` to the test board's address from the scan, which is a CoreBluetooth UUID on macOS. The check replaces the current score with a test fixture, confirms its generation and playback progress, reconnects, and stops playback using a fresh command sequence.

## Browser controls

Open `/chord-prompter/` on the MIRUI gallery host in Chrome or Arc on desktop, or Chrome on Android. For local development, run `trunk serve` in MIRUI's `gallery/web` directory and open `http://localhost:8080/chord-prompter/`. HTTPS or localhost is required for Web Bluetooth. Click **Connect display** and select **MIRUI Chord** in the browser chooser.

Upload a `.md` file or edit the practice source. Click **Send to display**, wait for validation, then **Play**. Play arms a start 500 ms ahead on the device clock. Tempo changes apply on the next counted beat; bar jumps and rhythm overrides apply on the next bar. The desktop preview can advance to the next section by tapping its chord row. Pending changes mark the displayed tempo with an asterisk. Pause preserves the fractional beat; Stop clears the position and pending controls.

The board continues playback after disconnection. Reconnection reads its current state without replaying old commands. Incomplete, corrupt or invalid uploads preserve the active score. The console's animated beat is interpolated from device samples; it is not a measurement of LCD presentation latency or an audio synchronization guarantee.

## Song sheets

```md
---
title: Practice
key: C
bpm: 96
time: 4/4
strum: pop
---
--- Verse ---
[Keep] the rhythm | [Let] it ring @ 1 5
--- Chorus | strum:folk ---
[One] more [turn] @ 1/5
```

`|` separates bars. The groups after `@` correspond to those bars. `/` divides a bar equally between chord slots. Each `[word]` starts the lyric fragment for a chord slot. These anchors do not define per-character timing. Scale degrees 1–6 resolve in the selected major key; degrees 2, 3 and 6 are minor. Counted beat duration is `60/BPM`, including in 6/8. Four-beat strumming patterns scale to the whole bar and continue across chord changes.

Available rhythm names: `whole`, `quarter`, `pop`, `poprock`, `ballad`, `folk`, `rock`, `punk`, `reggae`, `shuffle`, `country`, `arpeggio`. Pattern timing retains the imported hundredth-beat offsets; shuffle uses 0.67-beat upstrokes.

Limits are 8192 source bytes, 128 bars, 256 chord slots, 16 sections, 128 source bytes per lyric fragment and 64 bytes per title or section label. A bundled 12px one-bit font supports ASCII and an explicit Chinese subset. Unsupported visible characters are rejected before replacing the active score; this is not an unrestricted Chinese font. The repertoire and font licenses are recorded in MIRUI's `src/gallery/demos/assets/prompter-charset.txt` and `prompter-font-source.json`.

The service UUID is `7e4b0001-8c20-4b6d-a52b-9f7021cf4420`. Full packet layouts and receipt semantics are documented in MIRUI's `gallery/examples/tools/chord-prompter-protocol.md`.

## Display layout

The 128×128 screen uses centered native bitmap glyphs: 36px for the active chord and 12px for lyrics. The chord background is green while BLE is connected and gray after disconnection. The small surface reserves its space for the chord, lyrics, next chord and rhythm; playback controls live in the browser.

The browser workbench uses the full viewport, a bottom playback controller and a toggled right editor. Upload progress reports acknowledged bytes and a separate validation stage. Controls remain available between upload chunks. Browser preview uses the same Rust parser and musical transport; display lyrics are associated only with a source successfully activated at the current device generation.

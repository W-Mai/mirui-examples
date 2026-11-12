#!/usr/bin/env python3
"""Run the ESP32-C3 offscreen diagnostic with Espressif QEMU."""

import argparse
from pathlib import Path
import re
import subprocess
import time


EXPECTED_FRAMES = [
    b"form_off",
    b"form_on",
    b"form_slider_max",
    b"form_region_full",
    b"form_region_switch",
    b"switch_only_off",
    b"switch_only_on",
    b"switch_only_region",
]


def completed(output):
    if re.search(rb"\[PANIC\]|panicked at|abort\(\) was called", output, re.IGNORECASE):
        raise RuntimeError("The ESP32-C3 firmware panicked")
    if not re.search(rb"^\[TEST\] done\r*\n", output, re.MULTILINE):
        return False
    if re.findall(rb"^\[FB_END\] frame=(\w+)\r*$", output, re.MULTILINE) != EXPECTED_FRAMES:
        raise RuntimeError("Missing or unexpected framebuffer captures")
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("qemu", type=Path, help="Espressif's qemu-system-riscv32")
    parser.add_argument("flash", type=Path)
    parser.add_argument("artifacts", type=Path)
    parser.add_argument("--timeout", type=float, default=45)
    args = parser.parse_args()

    artifacts = args.artifacts.resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    serial = artifacts / "serial.log"
    command = [
        str(args.qemu.resolve()), "-machine", "esp32c3", "-icount", "3",
        "-display", "none", "-monitor", "none", "-serial", f"file:{serial}",
        "-drive", f"file={args.flash.resolve()},if=mtd,format=raw,snapshot=on",
    ]
    with (artifacts / "qemu.log").open("wb") as log:
        with subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT) as proc:
            try:
                deadline = time.monotonic() + args.timeout
                while time.monotonic() < deadline:
                    if serial.exists() and completed(serial.read_bytes()):
                        print("ESP32-C3 firmware passed eight offscreen frame checks")
                        return
                    if proc.poll() is not None:
                        raise RuntimeError(f"QEMU exited early with status {proc.returncode}")
                    time.sleep(0.1)
                raise RuntimeError("Timed out waiting for the offscreen diagnostic")
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    try:
                        proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        proc.kill()


if __name__ == "__main__":
    main()

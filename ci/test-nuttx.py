#!/usr/bin/env python3
"""Boot the NuttX example and inspect the guest's VirtIO GPU output."""

import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time


FRAME_HEADER = b"P6\n640 480\n255\n"
FRAME_SIZE = 640 * 480 * 3
GUEST_ERROR = re.compile(
    rb"assertion failed|panicked at|panic!!!|open backend failed|"
    rb"command not found|stack overflow",
    re.IGNORECASE,
)
RUNNING_DEMO = re.compile(rb"^\s*\d+\s+\d+\s+.*\bmirui\s*$", re.MULTILINE)


@contextmanager
def qemu(command, serial, errors):
    with serial.open("wb") as output, errors.open("wb") as error:
        with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=output, stderr=error) as proc:
            try:
                yield proc
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    try:
                        proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        proc.kill()


def wait_for(check, describe, timeout=45):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = check()
        if result:
            return result
        time.sleep(0.2)
    raise RuntimeError(f"Timed out waiting for {describe}")


def qmp_command(stream, name, **arguments):
    stream.write(json.dumps({"execute": name, "arguments": arguments}).encode() + b"\n")
    while True:
        response = json.loads(stream.readline())
        if "event" not in response:
            if "error" in response:
                raise RuntimeError(f"QMP {name} failed: {response['error']}")
            return response["return"]


def capture(stream, path):
    qmp_command(stream, "screendump", filename=str(path))
    frame = path.read_bytes()
    pixels = frame.removeprefix(FRAME_HEADER)
    if len(pixels) != FRAME_SIZE or pixels == frame:
        raise RuntimeError(f"Unexpected format or size in {path}")
    return pixels


def has_content(frame, before):
    # Sample throughout the frame to distinguish the Widgets screen from a
    # blank framebuffer, console overlay, or a single changed scanline.
    colors = {frame[index:index + 3] for index in range(0, FRAME_SIZE, 3 * 31)}
    return len(colors) >= 8 and frame != before


def test_demo(proc, serial, endpoint, artifacts):
    def guest_output():
        output = serial.read_bytes()
        if proc.poll() is not None:
            raise RuntimeError(f"QEMU exited early with status {proc.returncode}")
        if match := GUEST_ERROR.search(output):
            raise RuntimeError(f"NuttX reported {match[0].decode(errors='replace')}")
        return output

    def send(command):
        proc.stdin.write(command + b"\n")
        proc.stdin.flush()

    wait_for(lambda: b"nsh>" in guest_output(), "NSH")
    send(b"ls /dev")
    wait_for(lambda: re.search(rb"^\s*fb0\s*$", guest_output(), re.MULTILINE), "/dev/fb0")

    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(5)
        client.connect(str(endpoint))
        with client.makefile("rwb", buffering=0) as stream:
            if "QMP" not in json.loads(stream.readline()):
                raise RuntimeError("Missing QMP greeting")
            qmp_command(stream, "qmp_capabilities")
            before = capture(stream, artifacts / "before.ppm")

            send(b"mirui &")
            wait_for(lambda: b"mirui [" in guest_output(), "the mirui task to start")

            def widget_frame(filename, previous):
                guest_output()
                pixels = capture(stream, artifacts / filename)
                return pixels if has_content(pixels, before) and pixels != previous else None

            first = wait_for(lambda: widget_frame("widgets.ppm", before), "the first Widgets frame")
            wait_for(lambda: widget_frame("animated.ppm", first), "an updated Widgets frame")

    send(b"ps")
    wait_for(lambda: RUNNING_DEMO.search(guest_output()), "the running mirui task")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("disk", type=Path)
    parser.add_argument("artifacts", type=Path)
    args = parser.parse_args()
    artifacts = args.artifacts.resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    serial = artifacts / "serial.log"

    # Use a short socket path for hosts with a small UNIX socket path limit.
    with tempfile.TemporaryDirectory(prefix="mirui-nuttx-", dir="/tmp") as tmp:
        endpoint = Path(tmp) / "qmp.sock"
        command = [
            "qemu-system-riscv32", "-semihosting", "-M", "virt", "-cpu", "rv32",
            "-smp", "8", "-bios", "none", "-kernel", str(args.kernel.resolve()),
            "-drive", f"file={args.disk.resolve()},format=qcow2,if=virtio,snapshot=on",
            "-device", "virtio-gpu-device,xres=640,yres=480,bus=virtio-mmio-bus.0",
            "-display", "none", "-serial", "stdio", "-monitor", "none",
            "-qmp", f"unix:{endpoint},server=on,wait=off",
        ]
        with qemu(command, serial, artifacts / "qemu.log") as proc:
            test_demo(proc, serial, endpoint, artifacts)

    print("NuttX Widgets started, updated the framebuffer, and remained running")


if __name__ == "__main__":
    main()

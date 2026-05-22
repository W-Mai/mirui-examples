#!/usr/bin/env python3
"""Collect Chrome-trace events streamed by mirui-esp32c3 over UART.

ESP demo's `esp_perf_sink` prints one `[trace] {...}` line per
PerfEvent at every summary window. This script grabs those lines and
writes them as a Perfetto-loadable JSON array, suitable for
https://ui.perfetto.dev.

Usage:
    python3 tools/esp-trace.py --port /dev/cu.usbmodem101 --out trace.json
    # Ctrl-C to stop and finalize the file.
"""
import argparse
import json
import signal
import sys
import time

import serial


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--port", required=True)
    p.add_argument("--baud", type=int, default=115200)
    p.add_argument("--out", default="trace.json")
    p.add_argument(
        "--seconds",
        type=float,
        default=0,
        help="stop after this many seconds of capture (0 = run until Ctrl-C)",
    )
    args = p.parse_args()

    s = serial.Serial(args.port, args.baud, timeout=1)
    s.read(8192)

    events: list[dict] = []
    stop = {"flag": False}

    def handle_sigint(_sig, _frame):
        stop["flag"] = True

    signal.signal(signal.SIGINT, handle_sigint)

    start = time.monotonic()
    try:
        while not stop["flag"]:
            if args.seconds and time.monotonic() - start >= args.seconds:
                break
            line = s.readline()
            if not line:
                continue
            text = line.decode("utf-8", errors="replace").strip()
            if not text.startswith("[trace] "):
                continue
            payload = text[len("[trace] ") :]
            try:
                ev = json.loads(payload)
            except json.JSONDecodeError as e:
                print(f"warn: skipping malformed event: {e}", file=sys.stderr)
                continue
            events.append(ev)
    finally:
        with open(args.out, "w") as f:
            json.dump(events, f)
        print(f"wrote {len(events)} events → {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())

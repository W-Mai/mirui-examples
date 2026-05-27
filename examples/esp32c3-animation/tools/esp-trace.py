#!/usr/bin/env python3
"""Collect Chrome-trace events streamed by mirui-esp32c3 over UART.

Reads `[trace] {...}` lines (one per PerfEvent, emitted by
`esp_perfetto_box` when the demo is built with `--features
perf-trace`) and writes them to disk in either:

  - JSON array  — single `[ev, ev, ...]` document, drag-drop loadable
                  in https://ui.perfetto.dev. Default. Buffered until
                  the script exits, so long captures stay in RAM.
  - NDJSON      — one event per line, flushed incrementally. Survives
                  Ctrl-C without rewriting the whole file. Convert to
                  Perfetto-array form with `jq -s . trace.ndjson`.

Usage:
    python3 tools/esp-trace.py --port /dev/cu.usbmodem101
    python3 tools/esp-trace.py --port /dev/cu.usbmodem101 --ndjson --out long.ndjson
    python3 tools/esp-trace.py --port /dev/cu.usbmodem101 --seconds 30
"""
import argparse
import json
import signal
import sys
import time
from collections import Counter

import serial


def main() -> int:
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    p.add_argument("--port", required=True)
    p.add_argument("--baud", type=int, default=115200)
    p.add_argument("--out", default=None, help="output path (default: trace.json or trace.ndjson)")
    p.add_argument("--ndjson", action="store_true", help="write incrementally as NDJSON")
    p.add_argument(
        "--seconds",
        type=float,
        default=0,
        help="stop after this many seconds (0 = run until Ctrl-C)",
    )
    p.add_argument(
        "--quiet",
        action="store_true",
        help="suppress the 1Hz progress line",
    )
    args = p.parse_args()

    out_path = args.out or ("trace.ndjson" if args.ndjson else "trace.json")

    s = serial.Serial(args.port, args.baud, timeout=1)
    s.read(8192)

    stop = {"flag": False}

    def handle_sigint(_sig, _frame):
        stop["flag"] = True

    signal.signal(signal.SIGINT, handle_sigint)

    name_counts: Counter[str] = Counter()
    start = time.monotonic()
    last_progress = start
    count = 0

    # NDJSON: stream-write. Array mode: collect then dump once.
    f = open(out_path, "w", buffering=1) if args.ndjson else None
    events: list[dict] = []

    try:
        while not stop["flag"]:
            now = time.monotonic()
            if args.seconds and now - start >= args.seconds:
                break
            line = s.readline()
            if line:
                text = line.decode("utf-8", errors="replace").strip()
                if text.startswith("[trace] "):
                    payload = text[len("[trace] "):]
                    try:
                        ev = json.loads(payload)
                    except json.JSONDecodeError as e:
                        print(f"warn: skipping malformed event: {e}", file=sys.stderr)
                    else:
                        count += 1
                        name = ev.get("name", "?")
                        name_counts[name] += 1
                        if f is not None:
                            f.write(payload + "\n")
                        else:
                            events.append(ev)
            if not args.quiet and now - last_progress >= 1.0:
                elapsed = now - start
                print(
                    f"  [{elapsed:6.1f}s] {count:7d} events ({count / elapsed:6.0f}/s)",
                    file=sys.stderr,
                )
                last_progress = now
    finally:
        if f is not None:
            f.close()
        else:
            with open(out_path, "w") as fout:
                json.dump(events, fout)
        elapsed = time.monotonic() - start
        print(
            f"\ncaptured {count} events in {elapsed:.1f}s → {out_path}",
            file=sys.stderr,
        )
        if name_counts:
            print("top spans:", file=sys.stderr)
            for name, n in name_counts.most_common(8):
                print(f"  {n:7d}  {name}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())

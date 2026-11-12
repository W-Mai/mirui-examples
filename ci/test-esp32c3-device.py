#!/usr/bin/env python3
"""Check the running ESP32-C3 firmware on an explicitly selected device."""

import argparse
import asyncio
import re
import struct
import time


SERVICE = "7e4b0001-8c20-4b6d-a52b-9f7021cf4420"
CONTROL = "7e4b0002-8c20-4b6d-a52b-9f7021cf4420"
STATUS = "7e4b0003-8c20-4b6d-a52b-9f7021cf4420"
UPLOAD = "7e4b0004-8c20-4b6d-a52b-9f7021cf4420"
SONG = b"---\ntitle: Hardware CT\nkey: C\nbpm: 60\ntime: 4/4\nstrum: quarter\n---\n" + (
    b"[Keep] moving @ 1\n" * 16
)


def check_lcd(port):
    import serial

    deadline = time.monotonic() + 25
    active = False
    flush_windows = 0
    with serial.Serial(port, 115200, timeout=0.5, exclusive=True) as console:
        while time.monotonic() < deadline:
            line = console.readline().decode("utf-8", errors="replace").strip()
            if line:
                print(line, flush=True)
            if "[PANIC]" in line or "panicked at" in line:
                raise RuntimeError("Animation firmware panicked")
            if line.startswith("[perf] ") and " operations " in line:
                active = line.startswith("[perf] Tick operations ")
            if active and re.match(r"\[phase\] Flush \d+ns samples=[1-9]\d*$", line):
                active = False
                flush_windows += 1
                if flush_windows == 2:
                    print("Animation completed LCD flushes in two tick reports")
                    return
    raise RuntimeError("No sustained LCD flush activity from the animation firmware")


class Prompter:
    def __init__(self, client):
        self.client = client
        self.sequence = 0
        self.generation = 0

    async def status(self):
        data = await self.client.read_gatt_char(STATUS)
        if len(data) != 20 or data[0] != 1:
            raise RuntimeError("Invalid prompter status")
        generation, tick = struct.unpack_from("<II", data, 4)
        return data[1], generation, tick

    async def request(self, opcode, body, characteristic):
        packet = (struct.pack("<BH", opcode, self.sequence) + body).ljust(20, b"\0")
        await self.client.write_gatt_char(characteristic, packet, response=True)
        receipt = await self.client.read_gatt_char(CONTROL)
        if len(receipt) != 20:
            raise RuntimeError("Invalid prompter receipt length")
        result, received_opcode, received_sequence, generation = struct.unpack_from("<BBHI", receipt)
        next_sequence, = struct.unpack_from("<H", receipt, 16)
        if (result, received_opcode, received_sequence, next_sequence) != (
            0, opcode, self.sequence, (self.sequence + 1) & 0xffff
        ):
            raise RuntimeError(f"Invalid receipt for opcode {opcode}: {receipt.hex()}")
        self.sequence = next_sequence
        self.generation = generation

    async def control(self, opcode, argument=0, checksum=0):
        await self.request(opcode, struct.pack("<IBII", self.generation, 0, argument, checksum), CONTROL)

    async def upload(self, source):
        checksum = 0x811c9dc5
        for byte in source:
            checksum = ((checksum ^ byte) * 0x01000193) & 0xffffffff
        await self.control(0x10, len(source), checksum)
        for offset in range(0, len(source), 14):
            chunk = source[offset:offset + 14]
            await self.request(0x11, struct.pack("<HB", offset, len(chunk)) + chunk, UPLOAD)
        await self.control(0x12)


async def find_prompter(address):
    from bleak import BleakScanner

    device = await BleakScanner.find_device_by_address(address, timeout=8)
    if device is None:
        raise RuntimeError(f"Prompter {address} is not advertising")
    return device


async def check_ble(address):
    from bleak import BleakClient

    async with BleakClient(await find_prompter(address)) as client:
        if client.services.get_service(SERVICE) is None:
            raise RuntimeError("Missing prompter BLE service")
        prompter = Prompter(client)
        _, initial_generation, _ = await prompter.status()
        prompter.generation = initial_generation
        await prompter.upload(SONG)
        generation = prompter.generation
        if generation == initial_generation:
            raise RuntimeError("Uploaded score did not activate")
        await prompter.control(1, 500)
        deadline = time.monotonic() + 8
        while True:
            flags, active_generation, tick = await prompter.status()
            if active_generation == generation and flags & 1 and tick > 0:
                print(f"Score generation {generation} is playing at tick {tick}", flush=True)
                break
            if time.monotonic() >= deadline:
                raise RuntimeError("Playback did not advance after upload")
            await asyncio.sleep(0.2)

    async with BleakClient(await find_prompter(address)) as client:
        prompter = Prompter(client)
        flags, active_generation, resumed_tick = await prompter.status()
        if active_generation != generation or not flags & 1 or resumed_tick < tick:
            raise RuntimeError("Reconnection lost score or playback state")
        prompter.generation = generation
        await prompter.control(3)
        flags, active_generation, stopped_tick = await prompter.status()
        if flags & 1 or active_generation != generation or stopped_tick != 0:
            raise RuntimeError("Stop after reconnect did not reset playback")
        print(f"Reconnected at tick {resumed_tick}; a new control sequence stopped playback")


async def scan():
    from bleak import BleakScanner

    for device, advert in (await BleakScanner.discover(return_adv=True)).values():
        if (advert.local_name or device.name) == "MIRUI Chord":
            print(device.address)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="check", required=True)
    lcd = commands.add_parser("lcd", help="check SPI flush activity over USB serial")
    lcd.add_argument("port")
    commands.add_parser("scan", help="list prompter BLE addresses without connecting")
    ble = commands.add_parser("ble", help="check upload, playback and reconnect")
    ble.add_argument("address")
    args = parser.parse_args()

    if args.check == "lcd":
        check_lcd(args.port)
    elif args.check == "scan":
        asyncio.run(scan())
    else:
        asyncio.run(asyncio.wait_for(check_ble(args.address), timeout=45))


if __name__ == "__main__":
    main()

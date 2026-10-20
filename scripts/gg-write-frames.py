#!/usr/bin/env python3
"""Extract GG's on-board profile write frames from a USBPcap capture of official SteelSeries GG.

Reads a pcapng file, keeps the control transfers a header value selects, and prints the profile-file
write session in wire order: the `88` file open, then for every 1024-byte window two
`03` buffer writes (offsets 0 and 512) and one `05` push, then the `eb` commit. Frames are reported
as the data stage, i.e. the bytes after the report number hidapi puts in byte 0 of the buffer.

Output is a manifest the crate's tests compare their own write plan against: one line per frame
(index, report kind, wire length, FNV-1a 64 of the frame's bytes, head bytes as hex). The digest is
the same 64-bit FNV-1a the read side records block digests with
(`steelseries_gg::devices::keyboards::apex_7_profile::fnv1a_64`), so the fixture needs no new
dependency.

Usage:  python3 -I scripts/gg-write-frames.py <capture.pcapng> <header-filter-value> > FRAME_MANIFEST
The second argument is the header value the filter keeps: 8 for the reference capture, whose keyboard's
control transfers all carry it (see `control_frames`). The write session is the frames between the first
`88` file-open frame and the `eb` commit frame that follows it.
Print the first control frames with  python3 -I scripts/gg-write-frames.py <capture.pcapng> 8 --devices

Add  --full=3,35  to print those frames' complete bytes as hex after the digest. A manifest the crate
differs against uses it for the frames its own payload cannot reproduce byte for byte.
"""

import struct
import sys

FNV_OFFSET = 0xcbf29CE484222325
FNV_PRIME = 0x1000000001B3
MASK = (1 << 64) - 1

EPB = 6  # pcapng Enhanced Packet Block


def blocks(path):
    with open(path, "rb") as handle:
        while True:
            header = handle.read(8)
            if len(header) < 8:
                return
            block_type, total = struct.unpack("<II", header)
            body = handle.read(total - 12)
            handle.read(4)
            if block_type == EPB:
                timestamp = (int.from_bytes(body[4:8], "little") << 32) | int.from_bytes(body[8:12], "little")
                captured = struct.unpack("<I", body[12:16])[0]
                yield timestamp / 1e6, body[20 : 20 + captured]


def control_frames(path, device):
    """SET_REPORT control transfers, as (timestamp, wValue, wIndex, wLength, data stage).

    USBPcap's header is 28 bytes (`packet[0]` is its length). `device` filters bytes 18..20 of that
    header, read big-endian: in the GG write capture that is the value 8 on every packet carrying a control
    transfer's 8-byte setup record, and the header's own device fields are not decoded here - the capture
    holds one keyboard, so the filter selects the whole session. The frame type is not trusted as the
    filter: a control transfer is picked by its payload, which starts with the setup packet's
    `bmRequestType 0x21` (class, out) and a `bRequest` of 9 (SET_REPORT).
    """
    frames = []
    for timestamp, packet in blocks(path):
        if len(packet) < 28:
            continue
        header_len = packet[0]
        address = int.from_bytes(packet[18:20], "big")
        payload = packet[header_len:]
        if address != device or len(payload) < 8:
            continue
        if payload[0] != 0x21 or payload[1] not in (8, 9):
            continue
        w_value, w_index, w_length = struct.unpack("<HHH", payload[2:8])
        frames.append((timestamp, w_value, w_index, w_length, payload[8 : 8 + w_length]))
    return frames


def fnv1a_64(data):
    digest = FNV_OFFSET
    for byte in data:
        digest = ((digest ^ byte) * FNV_PRIME) & MASK
    return f"{digest:016X}"


def label(data):
    command = data[0]
    if command == 0x88:
        return f"open file {int.from_bytes(data[2:4], 'little'):04X}"
    if command == 0x03:
        return f"write buffer {int.from_bytes(data[2:4], 'little')} size {int.from_bytes(data[4:6], 'little')}"
    if command == 0x05:
        return (
            f"push file {int.from_bytes(data[2:4], 'little'):04X} "
            f"offset {int.from_bytes(data[4:8], 'little')} size {int.from_bytes(data[8:10], 'little')}"
        )
    if command == 0xEB:
        return f"commit {data[1]:02X}"
    return f"frame {command:02x}"


def main():
    positional = [arg for arg in sys.argv[1:] if not arg.startswith("--")]
    if len(positional) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    path, device = positional[0], int(positional[1])
    full = {
        int(number)
        for arg in sys.argv[1:]
        if arg.startswith("--full=")
        for number in arg.split("=", 1)[1].split(",")
        if number.strip()
    }
    frames = control_frames(path, device)
    if "--devices" in sys.argv[1:]:
        for timestamp, w_value, w_index, w_length, data in frames[:20]:
            print(f"{w_value:#07x} wIndex {w_index} wLength {w_length}  {data[:8].hex(' ')}")
        return 0

    start = next((i for i, f in enumerate(frames) if f[4][:1] == b"\x88"), None)
    if start is None:
        print("no 88 file-open frame in this capture", file=sys.stderr)
        return 1
    end = next((i for i, f in enumerate(frames[start:], start) if f[4][:1] == b"\xeb"), None)
    if end is None:
        print("no eb commit frame after the file open", file=sys.stderr)
        return 1

    print("# GG's on-board profile write session, one line per frame, in wire order.")
    print(f"# frames: {end - start + 1}  (file open, windows of 2 buffer writes + 1 push, commit)")
    print("# columns: index  report  wire_len  fnv1a_64  head16  label")
    for index, (_ts, w_value, _w_index, w_length, data) in enumerate(frames[start : end + 1], start=1):
        kind = "feature" if w_value == 0x0300 else "output"
        head = data[: min(16, len(data))].hex(" ")
        tail = f"  full={data.hex()}" if index in full else ""
        print(f"{index:>4}  {kind:<7}  {w_length:>4}  {fnv1a_64(data)}  {head:<47}  {label(data)}{tail}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

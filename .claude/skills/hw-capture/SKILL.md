---
name: hw-capture
description: Capture and decode what official SteelSeries GG sends to a device (USBPcap + tshark) or trace the GG engine's HID calls (Frida), to verify protocol claims on Windows. Use when asked to capture GG traffic, verify a wire format, or check whether GG issues a read-back.
user-invocable: true
disable-model-invocation: true
allowed-tools: Read Grep Glob Bash
---

Collect wire evidence of what GG does. Observe only: never modify GG, its files, or the device.

## Before running

1. Say which question the capture answers. Pick one method below; do not run all three.
2. Keyboard captures record every key press. Tell the user not to type passwords or private text.
3. Output goes outside the repo (`C:\Users\Ven0m0\gg-capture-artifacts\`), never into `src/` or `docs/`.

## A. Wire capture (USBPcap + tshark)

For guided, per-step capture of a new device, use `scripts/capture-device.ps1` (see `docs/development/adding-hardware.md`). For a single manual capture:

```
"C:\Program Files\USBPcap\USBPcapCMD.exe" -d \\.\USBPcap1 -o <file>.pcap -A
```

Always pass `-d`, `-o` and `-A`: with no arguments USBPcapCMD is interactive and hangs. The device address changes with the USB port; find it with tshark before filtering.

Decode lighting and profile uploads (feature reports):

```
"C:\Program Files\Wireshark\tshark.exe" -r <file>.pcap -Y "usb.setup.bRequest==9 && usb.bmRequestType==0x21 && usb.setup.wValue==0x0300" -T fields -e frame.time_epoch -e usb.data_fragment
```

Use `wValue==0x0200` for 64-byte output reports (commands). Replies arrive on interrupt endpoint `0x82`.

## B. Live engine trace (Frida)

Use when the question is "does GG send a command for this UI action?" or "does a read-back exist?". Frida 17 is installed (`frida`, `frida-trace`); the `rea` MCP (`start_frida_session`, `instrument_with_frida`) is the alternative.

- Target `SteelSeriesEngine.exe`, GG running, device attached.
- Hook kernel32 `ReadFile`, `WriteFile`, `DeviceIoControl`, e.g. `frida-trace -p <pid> -i "kernel32.dll!WriteFile" -i "kernel32.dll!DeviceIoControl"`. [UNVERIFIED invocation: adjust to the Frida 17 CLI if flags differ.]
- Log small buffers only. Earlier attempts that hooked large I/O coincided with the engine restarting on its own; the small-buffer trace was stable.
- Do the UI action while tracing, then look for `GET_FEATURE` (read-back) and for output reports other than the known poll set `02 01 80`, `b1`, `b2`, `34`, `90`.
- Heap reads (`ReadProcessMemory`) recover the engine's decoded live settings JSON. No decryption is needed or attempted.

## C. GG's stored config

Stop GG first (kill every `SteelSeries*` process or the engine respawns), then open read-only:
`C:\ProgramData\SteelSeries\GG\apps\engine\db\database.db`, table `configurations`, column `settings`.
Restart GG: from `C:\Program Files\SteelSeries\GG` run `SteelSeriesGGEZ.exe -dataPath="C:\ProgramData\SteelSeries\GG" -dbEnv=production`. `SteelSeriesGG.exe` alone exits with code 2.

## Static analysis (only if tracing cannot answer)

Ghidra is not on PATH on this machine. For `HIDDLL.dll` / `SSEdevice.dll` use `analyzeHeadless` if installed, otherwise the `rea` MCP Rizin/Cutter tools. `*.edevice` files are encrypted; do not try to decrypt them.

## Report

Give the file written, the exact bytes that answer the question (frame number or offset), and which status in `.claude/rules/experimental-protocol.md` the evidence supports. A capture alone never makes a frame Confirmed; that needs replay from `ssgg` on hardware.

# Adding support for your SteelSeries device

ssgg only knows the protocol of devices someone has captured. If your device is missing or only half works,
a 10-minute capture of SteelSeries GG talking to it is the most useful thing you can contribute. You do not
need to write Rust for that: open an issue with the capture and someone can implement it. If you do want to
write the code, see [From capture to pull request](#from-capture-to-pull-request).

## 1. Record a capture (Windows)

You need Windows, SteelSeries GG installed, and the device plugged in by USB (a wireless dongle counts).

```powershell
git clone https://github.com/Ven0m0/steelseriesgg-rs
cd steelseriesgg-rs
powershell -ExecutionPolicy Bypass -File .\scripts\capture-device.ps1 -DeviceType keyboard
```

`-DeviceType` is `keyboard`, `mouse`, `headset` or `other`. The script:

1. Asks for administrator rights (USB capture needs them).
2. Installs Wireshark and USBPcap with `winget` if they are missing. After a fresh USBPcap install,
   **reboot and run the script again**.
3. Lists the connected SteelSeries devices and lets you pick one.
4. Opens a window that stays on top. Click **START**: GG is closed, the capture starts, and the first step
   asks you to start GG again (this records the device initialisation). Then do each step in GG and click
   **DONE** once GG shows the change. Click **SKIP** for settings your device does not have. "Idle" steps
   advance on their own.
5. Writes a folder `ssgg-capture-<pid>-<date>` in your home directory.

Change only what the current step says, and only once. Every step is timestamped, so a stray click ends up
labelled as the wrong setting.

Options: `-SkipStartup` keeps GG running (no init capture), `-StepsFile my-steps.txt` uses your own steps
(one per line; a line starting with `Idle` waits instead of asking), `-SettleSeconds` and `-IdleSeconds`
change the wait times.

### Privacy

USBPcap records everything the device sends, including **key presses on a keyboard**. Do not type passwords or
private text while the capture runs. The script cuts the capture down to the selected device (other USB
devices are removed) and leaves the standard input endpoint (`0x81`, typed keys / mouse movement) out of
`reports.csv` and `summary.md`, but `device.pcap` still contains it.

### What you get

| File | Content |
|------|---------|
| `summary.md` | Per step: every distinct report sent or received, with counts. Start reading here. |
| `reports.csv` | Every report with timestamp, step, channel (`SET_REPORT feature id=.. if=..`, `interrupt ep ..`) and hex data |
| `steps.csv` | Step timestamps, including which steps were skipped |
| `device-info.txt` | USB interfaces and HID collections of the device, plus `ssgg devices` output if ssgg is built |
| `device.pcap` | The full capture of the device, for Wireshark |
| `ssgg-capture-<pid>.zip` | Everything above except `device.pcap` |

## 2. Share it

Open a [device capture issue](https://github.com/Ven0m0/steelseriesgg-rs/issues/new?template=device-capture.yml)
and attach the zip. Attach `device.pcap` too if it is under 25 MB (zip it first), or link it from a file host.

## From capture to pull request

1. Read `summary.md`. Look for the step where a setting changed and compare it with the steps before and
   after: the bytes that differ are the setting. [apex-pro-tkl-2023-gg-capture.md](apex-pro-tkl-2023-gg-capture.md)
   is a worked example (lighting frame, profile blob, per-key slots).
2. Write your findings as `docs/development/<device>-gg-capture.md` in the same style: transport (interface,
   report type and length), each command with its layout, and open questions.
3. Add the product ID to `product_ids` in `src/devices/mod.rs`. A keyboard then gets one row in
   `KEYBOARD_MODELS` (`src/devices/keyboards/models.rs`): name, class, raw-hidraw fallback and direct frame,
   which is everything discovery and the direct-lighting class read per model. A headset or mouse needs an
   arm in `device_name_from_product_id` and `device_type_from_product_id` (`src/devices/mod.rs`), wiring in
   `src/devices/discovery.rs`, and its own family table under `src/devices/headsets/` or `src/devices/mice/`.
   Run `node tools/profile-gui/sync-products.mjs` afterwards. The `/new-device`
   Claude Code command scaffolds this. Build every report with `HidReportBuilder`
   (`src/devices/hid_reports.rs`), never with hand-written byte arrays.
4. Label everything you have not replayed on the real device `[EXPERIMENTAL]` (see
   `.claude/rules/experimental-protocol.md`). Once it works on your hardware, say so in the PR with the
   device model and date.
5. Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings` and
   `cargo test --locked`, then open the PR and link the capture issue.

Do not write on-board profile or firmware data that the capture does not fully explain (for example a blob
with an unsolved checksum). Lighting is usually safe to try; profile writes can leave the device in a bad state.

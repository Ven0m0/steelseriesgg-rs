# steelseriesgg-rs – SteelSeries GG for Linux

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2FVen0m0%2Fsteelseriesgg-rs%2Fmain%2FCargo.toml&query=%24.package%5B%27rust-version%27%5D&label=rust&color=orange&style=flat-square)](https://www.rust-lang.org/)
![GitHub repo size](https://img.shields.io/github/repo-size/Ven0m0/steelseriesgg-rs)

Open-source [SteelSeries GG](https://steelseries.com/gg/engine) replacement for Linux. Control SteelSeries keyboards and headsets: RGB lighting, GameSense-compatible server, profiles, and (optional) audio/Sonar integration.

## Quickstart

```bash
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs
cargo build -r
./target/release/ssgg devices
```

To run the daemon as a user service after installing the package:

```bash
systemctl --user daemon-reload
systemctl --user enable --now ssgg.service
```

## Features

- **RGB lighting**: static, breathing, spectrum, wave, reactive, gradient, custom per-zone, off
- **GameSense server**: HTTP API compatible with SteelSeries GameSense
- **Profiles**: save/load device configurations
- **Daemon mode**: background service with animations and GameSense overlays
- **Audio mixer** (feature `audio`): domain model, PulseAudio/PipeWire wiring planned
- **Sonar API** (feature `sonar`): control SteelSeries Sonar via HTTP (Sonar must be running)

## Supported Devices

### Keyboards
- Apex Pro / Apex Pro TKL / Apex Pro TKL 2023 (PID `0x1630` - pending hardware confirmation)
- Apex 3 / Apex 3 TKL
- Apex 5
- Apex 7 / Apex 7 TKL
- Experimental, never run on hardware (community-fork protocol data): Apex 9 TKL / Mini, Apex M750 (solid colour only),
  Apex 5 / Apex 7 TKL / Apex Pro TKL / Apex Pro 2024 lighting

### Mice (experimental, `ssgg mouse`)
- Aerox 3 / 5 / 9 (incl. wireless), Prime Mini / Prime Wireless, Rival 3 Gen 2, Rival 5: DPI, poll rate, colour, battery.
  Settings are not saved to the mouse's memory.

### Headsets
- Arctis 1 / Arctis 1 Wireless
- Arctis 5 / Arctis 7 / Arctis 7 (2019 Edition)
- Arctis 9 / Arctis Pro / Arctis Pro Wireless
- Arctis Nova Pro / Arctis Nova Pro Wireless
- Arctis Nova Pro Omni (PID `0x2290` - pending hardware confirmation)
- Arctis Nova 5 / Arctis Nova 3 / Arctis Nova 1
- Arctis Nova 7 family (experimental, `ssgg headset`: status, sidetone, auto-off, mic volume; PID `0x22A1` and 14 others;
  all four confirmed on the Nova 7 Gen 2 `0x227E`)

## Installation

### Prerequisites

- Linux with udev
- Rust stable at or above the MSRV in the badge above (`rust-version` in `Cargo.toml`; `rust-toolchain.toml` tracks stable)

No extra HID development packages are required for the default source build.

### Arch package

```bash
sudo pacman -U ssgg-*.pkg.tar.zst
```

What you get: `/usr/bin/ssgg`, systemd user unit, udev rules, docs.

After install:
```bash
sudo usermod -aG input $USER
sudo udevadm control --reload-rules
sudo udevadm trigger
systemctl --user daemon-reload
systemctl --user enable --now ssgg.service   # optional
```
For boot without login: `sudo loginctl enable-linger $USER`.

### Build from source

```bash
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs
cargo build --release
```

For the optional `audio` feature on Debian/Ubuntu, install `libpulse-dev` first:

```bash
sudo apt install libpulse-dev
```

Binary: `target/release/ssgg`.

The release profile uses link-time optimization, symbol stripping, and single codegen unit for an optimized binary.

### Optional Build Optimizations

The project includes optional build optimizations in `.cargo/config.toml`. These are not required but can improve build times:

**sccache** (compilation cache):
```bash
cargo install sccache
# Then uncomment the rustc-wrapper line in .cargo/config.toml
```

**lld** (faster linker):
```bash
# Debian/Ubuntu
sudo apt install lld

# Fedora
sudo dnf install lld

# Arch Linux
sudo pacman -S lld

# Then uncomment the lld line in .cargo/config.toml
```

### Device permissions (udev)

```bash
sudo rm -f /etc/udev/rules.d/99-steelseries.rules   # old name, if you installed it before
sudo cp assets/70-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input $USER
```
Logged-in desktop users get access through `uaccess` without the group. The `input` group is still
needed for the daemon running outside a login session; log out/in to apply the group change.

### Key remapper (optional, EXPERIMENTAL)

`ssgg remap run` grabs a SteelSeries keyboard, rewrites key events and re-emits them through uinput. It is
Linux-only, needs `cargo build --features remap`, and has not been run against real hardware yet. It can only
remap one key to another key or to a chord (up to 4 keys); there are no macros, text typing or commands.

```bash
sudo cp assets/70-steelseries.rules /etc/udev/rules.d/   # includes the /dev/uinput rule (group `input`)
sudo cp assets/ssgg-uinput.conf /etc/modules-load.d/     # load uinput at boot
sudo modprobe uinput
sudo udevadm control --reload-rules && sudo udevadm trigger
```

```toml
[remap]
device = "0x1628"        # optional USB product id filter; omit to use every SteelSeries keyboard

[remap.bindings]
caps_lock = "esc"
f13 = ["left_ctrl", "left_shift", "t"]   # chord: pressed in order, released in reverse
```

`ssgg remap list` validates and prints the bindings. Stop a running remapper with Ctrl-C, or hold left Ctrl +
right Ctrl + left Shift + right Shift for 2 seconds.
The keyboard is grabbed exclusively while `remap run` is active.

### Windows

Windows builds work but are not the primary target. No udev rules or driver changes are needed.

**Prebuilt binary**: download `ssgg-windows-x86_64.exe` and `sha256sums-windows.txt` from the
[latest release](https://github.com/Ven0m0/steelseriesgg-rs/releases/latest), then check the hash in PowerShell:

```powershell
(Get-FileHash .\ssgg-windows-x86_64.exe -Algorithm SHA256).Hash.ToLower()
Get-Content .\sha256sums-windows.txt
Rename-Item .\ssgg-windows-x86_64.exe ssgg.exe
.\ssgg.exe devices
```

Move `ssgg.exe` to a folder on your `PATH` to run it from anywhere.

**Build from source**: install [rustup](https://rustup.rs) and the Visual Studio Build Tools with the
"Desktop development with C++" workload (the `x86_64-pc-windows-msvc` target needs the MSVC linker), then:

```powershell
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs
cargo build --release --locked
.\target\release\ssgg.exe devices
```

Notes:

- Quit SteelSeries GG before using `ssgg rgb`. GG's engine keeps rewriting the lighting and overrides `ssgg`;
  `ssgg rgb` prints a warning while `SteelSeriesEngine.exe` is running. End every `SteelSeries*` process in Task
  Manager, or the engine restarts itself.
- The `audio` feature needs PulseAudio and does not build on Windows. `--features sonar` works.
- There is no Windows service; run `ssgg daemon` in a terminal or from Task Scheduler at logon.
- Config file: `%APPDATA%\steelseries-gg\ssgg\config\config.toml`.
- `ssgg pollrate status` reports "unsupported on this driver" for USB devices; Windows HID drivers do not expose the
  poll rate.

## Usage

### List devices

```bash
ssgg devices
```

### RGB lighting

Set a static color:
```bash
ssgg rgb --color red
ssgg rgb --color "#ff5500"
```

Set brightness:
```bash
ssgg rgb --brightness 80
```

Apply effects:
```bash
ssgg rgb --effect breathing --color cyan
ssgg rgb --effect spectrum
ssgg rgb --effect wave --direction left-to-right
```

Available effects: `static`, `breathing`, `spectrum`, `wave`, `reactive`, `gradient`, `off`

### Profiles

Save current configuration:
```bash
ssgg profile save my-profile
```

Load a profile:
```bash
ssgg profile load my-profile
```

List profiles:
```bash
ssgg profile list
```

### Audio mixer (feature `audio`)

View audio status:
```bash
ssgg audio status
```

Set volume:
```bash
ssgg audio volume --channel master --level 75
```

Mute/unmute:
```bash
ssgg audio mute --channel game
ssgg audio unmute --channel game
```

Adjust chat mix:
```bash
ssgg audio chat-mix --balance 25
```

### SteelSeries Sonar (feature `sonar`)

The Sonar integration provides direct control over the SteelSeries Sonar audio device through its HTTP API. Sonar must be running for these commands to work.

View Sonar status:
```bash
ssgg sonar status
```

Discover the dynamic Sonar API port:
```bash
ssgg sonar discover
```

List audio devices:
```bash
ssgg sonar devices
```

Set volume for a channel (classic mode):
```bash
ssgg sonar volume master 80
ssgg sonar volume game 100
ssgg sonar volume chat 75
```

Get current mode:
```bash
ssgg sonar mode
```

Control streamer mode volumes:
```bash
# Set monitoring volume
ssgg sonar streamer monitoring master 75
ssgg sonar streamer monitoring game 80

# Set streaming volume
ssgg sonar streamer streaming master 90
```

List available configurations:
```bash
ssgg sonar configs
```

### GameSense server

Start the GameSense HTTP server:
```bash
ssgg server
```

The server runs on port 27301 by default and is compatible with games that support SteelSeries GameSense.

### Daemon mode

Run as a background daemon with device control and GameSense server:
```bash
ssgg daemon
```

The daemon will run in the foreground and can be stopped gracefully with Ctrl+C or `systemctl --user stop ssgg.service` when running as a systemd service.

Run as systemd user service:
```bash
systemctl --user daemon-reload
systemctl --user enable --now ssgg.service
journalctl --user -u ssgg.service -f
```

### Device diagnostics and testing

View HID device logs:
```bash
ssgg hid-logs
```

Run device self-test:
```bash
ssgg test-device
```

Verify device performance metrics:
```bash
ssgg verify-performance
```

Run HID report fuzzing (development tool):
```bash
ssgg fuzz
```

## Configuration

Configuration is stored in `~/.config/ssgg/config.toml`:

```toml
[gamesense]
enabled = true
bind = "127.0.0.1"
port = 27301

[audio]
master_volume = 100
game_volume = 100
chat_volume = 100

[general]
default_profile = "default"
debug = false
```

## Feature Flags

- `audio` - Enable audio mixer with PulseAudio/PipeWire support
- `sonar` - Enable SteelSeries Sonar API integration
- `remap` - Enable the evdev key remapper (`ssgg remap run`, Linux only, experimental)

By default, no optional features are enabled. The Arch package ships with default features only.

Build with audio support:
```bash
cargo build --release --features audio
```

Build with all features:
```bash
cargo build --release --all-features
```

## Helper binaries

The project includes developer and testing utilities:

- `discover_actuation` - Probe actuation firmware commands (development tool)
- `sonar_control` - SteelSeries Sonar HTTP API exerciser (requires `sonar` feature)

## Dependencies

| Crate | Purpose |
|-------|---------|
| hidapi | HID device communication (exact version pin, see `Cargo.toml`) |
| tokio | Async runtime |
| axum | GameSense HTTP server |
| reqwest | Sonar HTTP client (optional, `sonar` feature) |
| serde / serde_json | Serialization |
| clap | CLI argument parsing |
| tracing / tracing-subscriber | Logging |
| thiserror / anyhow | Error handling |
| libpulse-binding | Audio integration (`audio` feature) |
| sysinfo | System diagnostics |
| tabled | Terminal table output |
| colored | Colored terminal output |
| indicatif | Progress indicators |
| chrono | Timestamps for diagnostics |

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Issues and PRs are welcome. Please run `cargo fmt && cargo clippy --all-targets --locked` before submitting.

Device not supported yet? Record a guided capture of SteelSeries GG with `scripts/capture-device.ps1` and
share it; see [docs/development/adding-hardware.md](docs/development/adding-hardware.md).

## Acknowledgments

- [hidapi](https://crates.io/crates/hidapi) - HID access
- [apex-tux](https://github.com/not-jan/apex-tux) and apex7tkl_linux - keyboard inspiration

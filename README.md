# steelseriesgg-rs – SteelSeries GG for Linux

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange?style=flat-square)](https://www.rust-lang.org/)
![GitHub repo size](https://img.shields.io/github/repo-size/Ven0m0/steelseriesgg-rs)
[![Maintainability](https://qlty.sh/gh/Ven0m0/projects/steelseriesgg-rs/maintainability.svg)](https://qlty.sh/gh/Ven0m0/projects/steelseriesgg-rs)

Open-source [SteelSeries GG](https://steelseries.com/gg/engine) replacement for Linux. Control SteelSeries keyboards and headsets: RGB lighting, GameSense-compatible server, profiles, and (optional) audio/Sonar integration.

## Quickstart

```bash
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs
cargo build --release
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
- **Daemon mode**: background service with animations + GameSense overlays
- **Audio mixer** (feature `audio`): domain model, PulseAudio/PipeWire wiring planned
- **Sonar API** (feature `sonar`): control SteelSeries Sonar via HTTP (Sonar must be running)

## Supported Devices **WIP**

### Keyboards
- Apex Pro / Apex Pro TKL / [Apex Pro TKL 2023](https://steelseries.com/gaming-keyboards/apex-pro-2023)
- Apex 3 / Apex 3 TKL
- Apex 5
- Apex 7 / Apex 7 TKL

### Headsets
- Arctis 1 / Arctis 1 Wireless
- Arctis 5 / Arctis 7 / Arctis 7 (2019 Edition)
- Arctis 9 / Arctis Pro / Arctis Pro Wireless
- Arctis Nova Pro / Arctis Nova Pro Wireless
- Arctis Nova 5 / Arctis Nova 3 / Arctis Nova 1

## Installation

### Prerequisites

- Linux with udev
- Rust 1.88+ (minimum supported for source builds)

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

### Optional Build Optimizations

The project includes optional build optimizations in `.cargo/config.toml`. These are **not required** but can improve build times:

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

These optimizations are experimental and optional. The project builds successfully without them.

### Device permissions (udev)

```bash
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input $USER
```
Log out/in to apply the group change.

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

**Run as systemd user service**

```bash
systemctl --user daemon-reload
systemctl --user enable --now ssgg.service
journalctl --user -u ssgg.service -f
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

## Dependencies (core crates)

| Crate | Purpose |
|-------|---------|
| hidapi | HID device communication |
| tokio | Async runtime |
| axum | GameSense HTTP server |
| reqwest | Sonar HTTP client |
| serde / serde_json | Serialization |
| clap | CLI argument parsing |
| tracing | Logging |
| libpulse-binding | Audio integration (feature `audio`) |

## Feature Flags

- `audio` - Enable audio mixer with PulseAudio support (optional)
- `sonar` - Enable SteelSeries Sonar API integration (optional, requires `audio`)

By default, no optional features are enabled. The Arch package ships with default features only.

Build with audio support:
```bash
cargo build --release --features audio
```

Build with all features:
```bash
cargo build --release --all-features
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Issues and PRs are welcome. Please run `cargo fmt && cargo clippy --all-targets --locked` before submitting.

## Acknowledgments

- [hidapi](https://crates.io/crates/hidapi) — HID access
- [apex-tux](https://github.com/not-jan/apex-tux) and apex7tkl_linux — keyboard inspiration

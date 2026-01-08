# steelseriesgg-rs – SteelSeries GG for Linux

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange?style=flat-square)](https://www.rust-lang.org/)

A complete open-source replacement for SteelSeries GG on Linux. Control your SteelSeries keyboards and headsets with RGB lighting, audio mixing, GameSense API compatibility, and profile management.

## Features

- **RGB Lighting Control** - Static colors, breathing, spectrum cycling, wave effects, reactive effects, and gradients
- **GameSense Server** - HTTP API compatible with SteelSeries GameSense for game integrations
- **Audio Mixer** - Per-channel volume control (PulseAudio/PipeWire integration is a work in progress)
- **Sonar API Integration** - Direct control of SteelSeries Sonar audio device (based on [GGSonarRev](https://github.com/PrzemekkkYT/GGSonarRev))
- **Profile Management** - Save and load device configurations
- **Daemon Mode** - Run as a background service

## Supported Devices

### Keyboards
- Apex Pro / Apex Pro TKL / Apex Pro TKL 2023
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

- Linux with udev support
- Rust toolchain (1.70+)
- `libudev-dev` or equivalent for your distribution
- `libhidapi-dev` for HID device communication

**Debian/Ubuntu:**
```bash
sudo apt install libudev-dev libhidapi-dev
```

**Fedora:**
```bash
sudo dnf install systemd-devel hidapi-devel
```

**Arch Linux:**
```bash
sudo pacman -S hidapi
```

### Building from Source

```bash
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs
cargo build --release
```

The binary will be located at `target/release/ssgg`.

### Device Permissions (udev rules)

To access SteelSeries devices without root, install the udev rules:

```bash
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Then add your user to the `input` group:

```bash
sudo usermod -aG input $USER
```

Log out and log back in for the group change to take effect.

## Usage

### List Connected Devices

```bash
ssgg devices
```

### RGB Lighting

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

### Profile Management

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

### Audio Mixer

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

### SteelSeries Sonar Control

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

### GameSense Server

Start the GameSense HTTP server:
```bash
ssgg server
```

The server runs on port 27301 by default and is compatible with games that support SteelSeries GameSense.

### Daemon Mode

Run as a background daemon with device control and GameSense server:
```bash
ssgg daemon
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

## Dependencies

| Crate | Purpose |
|-------|---------|
| hidapi | HID device communication |
| tokio | Async runtime |
| axum | HTTP server for GameSense |
| reqwest | HTTP client for Sonar API |
| serde | Serialization |
| clap | CLI argument parsing |
| libpulse-binding | PulseAudio integration (optional) |

## Feature Flags

- `audio` (default) - Enable audio mixer with PulseAudio support
- `compat` - Enable compatibility with existing SteelSeries crates

Build without audio:
```bash
cargo build --release --no-default-features
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

## Acknowledgments

- [hidapi](https://crates.io/crates/hidapi) - Cross-platform HID device library
- [apex-tux](https://github.com/not-jan/apex-tux), apex7tkl_linux - Inspiration for keyboard support

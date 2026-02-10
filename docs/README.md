# SteelSeries GG (Rust)

This project is a Rust implementation of the SteelSeries GG software. It provides a CLI and library for interacting with SteelSeries devices.

## Features

- Device discovery
- RGB control
- Device configuration

## Installation

For installation instructions, please refer to the main [README.md](../README.md) in the repository root.

Quick start:
```bash
# Clone the repository
git clone https://github.com/Ven0m0/steelseriesgg-rs.git
cd steelseriesgg-rs

# Build from source
cargo build --release

# Install udev rules (Linux only)
sudo cp assets/99-steelseries.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## Usage

### Basic Commands

List connected devices:
```bash
ssgg devices
```

Control RGB lighting:
```bash
# Set solid color
ssgg rgb color red
ssgg rgb color "#00FF00"

# Apply effects
ssgg rgb effect breathing --color cyan --speed 2.0
ssgg rgb effect spectrum --speed 1.5

# Adjust brightness
ssgg rgb brightness 80
```

### Daemon Mode

Run as background service:
```bash
ssgg daemon
```

Or use the systemd service:
```bash
systemctl --user enable --now ssgg.service
```

### For More Information

- Complete documentation: [AGENTS.md](../AGENTS.md)
- Development guide: [CONTRIBUTING.md](../CONTRIBUTING.md)
- Protocol research: [development/](development/)

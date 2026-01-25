# Project Plan: SteelSeries Apex Pro RGB Control

## 🎯 Goal
Achieve precise, per-key RGB lighting control for SteelSeries Apex Pro keyboards (specifically targeting the TKL 2023 variant) on Linux. The project aims to move beyond the current zone-based limitations to full, individual key addressability.

## 📊 Current Status
- **Working:**
  - Basic HID communication established.
  - Zone-based RGB control (9 zones for TKL 2023).
  - Global brightness control.
  - Settings application.
- **Missing:**
  - Per-key RGB addressing protocol (Unknown).
  - Key-to-address mapping table.
  - CLI commands for individual key control.

## 🗓️ Implementation Phases

### Phase 1: Discovery & Diagnostics (Current Focus)
*Objective: Uncover the hidden HID protocol details required for per-key control.*

- [ ] **US-002: HID Diagnostic Tool**
  - Implement `--debug-hid` flag to log raw reports.
  - Add CRC/Checksum validation logic.
  - Create a packet capture analyzer (if needed).
- [ ] **US-001: Protocol Reverse Engineering**
  - Analyze USB traffic (using Wireshark/usbmon) from SteelSeries Engine (Windows) to identify per-key packets.
  - Fuzzing: Systematically test undefined command bytes (e.g., beyond `0x21`, `0x22`).
  - Document findings in `docs/development/APEX_PRO_PROTOCOL.md`.

### Phase 2: Core Protocol Implementation
*Objective: Build the internal logic to construct valid per-key commands.*

- [ ] **US-003: Report Structure**
  - Define the Rust structs for per-key HID reports.
  - Implement header and padding logic discovered in Phase 1.
- [ ] **US-004: Key Mapping**
  - Create `KeyMapping` struct/enum.
  - Map physical ISO/ANSI keys to discovered HID addresses.
- [ ] **US-005: Command Builder**
  - Implement `build_per_key_command(key, color)`.
  - Implement batch command builder for efficiency.

### Phase 3: Integration & CLI
*Objective: Expose the new capabilities to the user.*

- [ ] **US-006: Trait Update**
  - Update `Keyboard` trait with `set_key_color` and `set_keys`.
  - Implement trait for `ApexProTKL2023`.
- [ ] **US-007: CLI Expansion**
  - Add `rgb set-key <KEY> <COLOR>` command.
  - Add `rgb test-keys` interactive mode.

### Phase 4: Polish & Advanced Features
*Objective: Ensure reliability and feature parity with official software.*

- [ ] **US-008: Zone Fallback**
  - Gracefully handle failures by falling back to zone mode if per-key fails.
- [ ] **US-009: Effect Engine**
  - Update `EffectEngine` to support per-key frames (Wave, Breathing, etc.).

### Phase 5: Verification
*Objective: Ensure stability.*

- [ ] **US-010: Testing**
  - Unit tests for key mapping.
  - Integration tests for command generation.

## 🛠️ Key Technical Resources
- `docs/development/APEX_PRO_PROTOCOL.md`: Protocol knowledge base.
- `src/devices/keyboards/apex_pro_tkl_2023.rs`: Primary target device file.

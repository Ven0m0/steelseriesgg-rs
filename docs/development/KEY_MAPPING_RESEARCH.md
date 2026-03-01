# SteelSeries Apex Pro Key Mapping Research

**Date**: 2026-03-01 (originally 2026-01-16)
**Status**: PLACEHOLDER MAPPINGS — Requires Hardware Research
**Priority**: Critical for per-key RGB implementation

## Overview

This document outlines the current state of key-to-address mapping research for SteelSeries Apex Pro keyboards. The goal is to enable precise per-key RGB control by understanding the HID key matrix addressing scheme.

## ⚠️ CRITICAL DISCLAIMER

**The key mappings currently implemented are PLACEHOLDER data based on standard keyboard layouts and educated guesses. They are NOT verified against real hardware and must NOT be relied upon for production use.**

Accurate HID key addresses must be discovered through hardware research and reverse engineering before per-key RGB can work correctly. In the meantime, the codebase falls back to zone-based RGB via `simulate_per_key_with_zones()`.

## Current Implementation

### Architecture

The key mapping system is implemented in `src/devices/key_mapping.rs` with:

- **`KeyId` enum**: 87 physical key identifiers covering function row, letter rows, bottom row, arrow cluster, navigation cluster, numpad, and SteelSeries-specific keys (`SteelSeriesKey`, `VolumeWheel`)
- **`KeyAddress` struct**: Matrix coordinates `(row, col)` for HID addressing
- **`KeyboardLayout` enum**: `FullSize`, `TenKeyLess`, `Compact`
- **`KeyMapping` struct**: Complete mapping for a specific keyboard model, including a `key_map: HashMap<KeyId, KeyAddress>`, declared matrix dimensions, and cached key lists for efficient iteration
- **`KeyMappingStats` struct**: Statistics about a mapping (total keys, matrix utilization, actual vs. declared dimensions)
- **`KeyMappingDatabase`**: Database of all supported keyboard mappings, populated at construction with placeholder data

### `KeyMapping` API

```rust
let mut mapping = KeyMapping::new(
    product_id,
    KeyboardLayout::TenKeyLess,
    "Apex Pro TKL (2023)".to_string(),
    6,   // matrix rows
    17,  // matrix columns
);

mapping.add_key(KeyId::A, KeyAddress::new(3, 1));
mapping.get_key_address(KeyId::A);    // → Some(KeyAddress { row: 3, col: 1 })
mapping.supports_key(KeyId::A);       // → true
mapping.get_all_keys();               // → &[KeyId] (stable iteration order)
mapping.get_stats();                  // → KeyMappingStats
```

### Supported Models (PLACEHOLDER)

| Model | Product ID | Layout | Matrix Size | Mapped Keys | Utilization | Status |
|-------|------------|--------|-------------|-------------|-------------|--------|
| Apex Pro TKL (2023) | `0x1628` | TenKeyLess | 6×17 | ~87 | ~85 % | PLACEHOLDER |
| Apex Pro | `0x1610` | Full-size | 6×21 | ~104 | ~82 % | PLACEHOLDER |
| Apex Pro TKL | `0x1614` | TenKeyLess | 6×17 | ~87 | ~85 % | PLACEHOLDER |

### Placeholder Matrix Layout (Apex Pro TKL 2023)

```
Row 0 (Function):  ESC _ F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12   INS HOM PGU
Row 1 (Numbers):    `  1  2  3  4  5  6  7  8  9  0   -   =  BKSP  DEL END PGD
Row 2 (QWERTY):   TAB  Q  W  E  R  T  Y  U  I  O  P   [   ]   \
Row 3 (Home):     CAPS  A  S  D  F  G  H  J  K  L  ;   '  _  ENTER        ↑
Row 4 (Shift):    LSHF  _  Z  X  C  V  B  N  M  ,  .   /  RSHF       ←   ↓   →
Row 5 (Bottom):   LCTL LWIN LALT _ _ _ SPC _ _ _ RALT RWIN MENU RCTL
```

> `_` indicates unused matrix positions (gaps). Real hardware matrices are typically 40–60 % utilized.

## Zone-Based Fallback (Working Alternative)

While per-key mapping uses placeholder data, the **zone fallback system** (`src/devices/zone_mapping.rs`) provides a working approximation:

- **`ZoneMapping`** associates zones with `ZonePosition` identifiers (`MainKeys`, `FunctionRow`, `ArrowKeys`, etc.)
- **`ZoneFallback`** maps individual `KeyId` values to their nearest zone
- **`simulate_per_key_with_zones()`** on the `Keyboard` trait converts per-key color requests into zone-based updates

This is the **recommended approach** until accurate per-key addresses are discovered.

## Research Requirements

### Phase 1: HID Descriptor Analysis

**Objective**: Understand the device's HID report structure

**Methods**:
1. **Linux**: `lsusb -v -d 1038:1628` to dump HID descriptors
2. **Analysis**: Parse HID report descriptor to find input/output report structure
3. **Tools**: `usbhid-dump -d 1038:1628`, hidapi test programs

**Expected Findings**:
- Report size (known: 65 bytes for keyboards)
- Input/output report types
- Usage pages and usage IDs
- Possible key addressing hints

### Phase 2: Protocol Analysis

**Objective**: Discover per-key RGB commands by analyzing existing software

**Methods**:
1. **USB Traffic Capture**: Use Wireshark/usbmon to monitor SteelSeries GG (via WINE/Lutris)
2. **HID Report Analysis**: Decode captured HID output reports to the device
3. **Pattern Recognition**: Identify per-key vs. zone command patterns

**Expected Findings**:
- Per-key RGB command format (likely different from the zone command `0x21`)
- Key addressing scheme (sequential, matrix row/col, or other)
- Command validation and acknowledgment patterns

### Phase 3: Hardware Testing

**Objective**: Empirically discover key addresses through controlled testing

**Methods**:
1. **Systematic Testing**: Send test commands to individual matrix positions via `bulk_test` binary
2. **Visual Verification**: Observe which physical keys light up
3. **Matrix Mapping**: Build accurate row/column → physical key mapping

**Tools Needed**:
- Apex Pro hardware (preferably TKL 2023, PID `0x1628`)
- The `bulk_test` binary (`src/bin/bulk_test.rs`) for automated command sweeps
- Physical observation and documentation

### Phase 4: Reverse Engineering

**Objective**: Understand SteelSeries proprietary protocol extensions

**Methods**:
1. **Traffic Capture**: Compare official software commands with our implementation
2. **Code Analysis**: Study apex-tux, apex7tkl_linux, msi-perkeyrgb Python/Rust code
3. **Documentation**: Search for leaked or published protocol specifications

**Risk**: May have IP/licensing implications — proceed with caution.

## Known Protocol Information

### Current Zone-Based RGB (Working)

```
Command 0x21: [0x00] [0x21] [0xFF] [R G B] [R G B] ... (up to 12 zones)
- Works for zone-based lighting
- Controls groups of keys, not individual keys
- Limited precision for effects
```

### Hypothetical Per-Key RGB (Unknown)

The placeholder `PerKeyRgbCommand` (command `0x23`) supports three addressing modes that **have not been verified**:

```rust
pub enum PerKeyAddressingMode {
    Sequential,    // Keys indexed 0..N
    MatrixRowCol,  // Keys addressed by (row, col)
    DirectIndex,   // Keys addressed by a single index
}
```

Possible real formats (speculative):
1. `0x23 [key_index] [R G B]` — Single key by index
2. `0x23 [row] [col] [R G B]` — Matrix addressing
3. `0x2X [batch_data...]` — Bulk key update
4. Completely different command code

### Actuation Point (Discovered)

```
Command 0x2D: [0x00] [0x2D] [value] [padding]
- value: 0.1mm increments (1 = 0.1mm, 36 = 3.6mm)
- Valid range: 1–40
- Write confirmed working; read command unknown
```

## Implementation Strategy

### Immediate (Development Structure) ✅ Completed

- [x] Key mapping data structures (`KeyId`, `KeyAddress`, `KeyMapping`, `KeyMappingDatabase`)
- [x] Database system for multiple keyboard variants
- [x] Placeholder mappings for development
- [x] Comprehensive test coverage
- [x] Integration with HID report system (`PerKeyRgbBuilder`, `PerKeyRgbCommand`)
- [x] Zone-based fallback system (`ZoneMapping`, `ZoneFallback`)

### Short-term (Research Phase) 🔄 In Progress

1. Complete observation analysis of bulk test results (0x20–0x2F range)
2. Capture USB traffic with SteelSeries GG via WINE/Lutris
3. Identify per-key command byte(s) from captured traffic
4. Build systematic key testing utilities
5. Document findings in this research log

### Long-term (Production Ready) ⏸️ Pending

1. Replace placeholder mappings with verified hardware data
2. Implement actual per-key RGB commands in `PerKeyRgbCommand`
3. Add runtime key mapping discovery
4. Create validation tools for mapping accuracy
5. Support ANSI and ISO layout variants

## Current Limitations

### Functional Limitations

- ❌ **No per-key control**: Zone-based RGB only (per-key uses placeholder protocol)
- ❌ **Inaccurate mappings**: Placeholder data only — not verified against hardware
- ❌ **No key discovery**: Cannot detect available keys at runtime
- ❌ **No validation**: Cannot verify mapping accuracy without hardware

### Research Limitations

- ❌ **Limited hardware access**: Development primarily without physical devices
- ❌ **No protocol docs**: Proprietary SteelSeries protocol
- ❌ **Legal constraints**: Reverse engineering restrictions may apply

## Integration Points

### Current Integration

| Component | File | Status |
|-----------|------|--------|
| HID Reports | `src/devices/hid_reports.rs` — `PerKeyRgbBuilder`, `PerKeyRgbCommand` | ✅ Ready (placeholder protocol) |
| Keyboard Trait | `src/devices/keyboards/mod.rs` — `set_key_color()`, `set_key_colors()`, `set_key_color_direct()` | ✅ API defined |
| Zone Fallback | `src/devices/zone_mapping.rs` — `simulate_per_key_with_zones()` | ✅ Working |
| Device Discovery | `src/devices/discovery.rs` — product ID based mapping selection | ✅ Working |
| Diagnostics | `src/devices/diagnostics.rs` — HID communication logging | ✅ Working |
| RGB Effects | `src/rgb/mod.rs` — `PerKeyEffect`, `PerKeyRgbController` | ✅ Ready |

### Future Integration (When Accurate Mappings Available)

- 🔄 **RGB Controller**: Per-key effect rendering with real matrix addresses
- 🔄 **GameSense Server**: Individual key reactive lighting
- 🔄 **Profiles**: Save/load per-key color configurations
- 🔄 **CLI Tools**: Per-key testing and debugging commands
- 🔄 **Device State**: Persist per-key colors in state file

## Research Log

### 2026-01-16: Initial Implementation

**Accomplished**:
- ✅ Complete key mapping data structure
- ✅ Database for multiple keyboard models
- ✅ Placeholder mappings (development only)
- ✅ Comprehensive test coverage (all tests passing)
- ✅ Integration with existing HID report system

**Findings**:
- Standard matrix addressing is typically 6–8 rows × 15–21 columns
- TKL keyboards usually use fewer columns than full-size
- Matrix utilization is typically 40–60 % (many positions unused)
- SteelSeries likely uses sparse matrix addressing

**Next Priority**: Create HID analysis tools for traffic capture

### Future Research Entries

```
### YYYY-MM-DD: [Research Phase]
**Objective**: [What was attempted]
**Methods**: [How research was conducted]
**Findings**: [What was discovered]
**Challenges**: [What problems were encountered]
**Next Steps**: [What should be tried next]
```

## Testing Matrix

### Development Testing (Current)

| Test | Status | Notes |
|------|--------|-------|
| Data structures | ✅ PASS | KeyId, KeyAddress, KeyMapping |
| Database loading | ✅ PASS | Known products detected via product_ids |
| Mapping stats | ✅ PASS | Statistics calculation works |
| Key identification | ✅ PASS | KeyId Display formatting verified |
| Zone fallback | ✅ PASS | simulate_per_key_with_zones() works |

### Hardware Testing (Future)

| Test | Status | Hardware Required |
|------|--------|-------------------|
| Matrix addressing | ❌ TODO | Apex Pro TKL 2023 (`0x1628`) |
| Per-key RGB commands | ❌ TODO | Any Apex Pro model |
| Command validation | ❌ TODO | Multiple Apex variants |
| Error handling | ❌ TODO | Test invalid addresses |
| ANSI vs ISO layout | ❌ TODO | Both layout variants |

## Success Criteria

### Phase 1 Success (Research Structure) ✅ COMPLETED

- [x] Complete key mapping data structure
- [x] Database system for multiple keyboards
- [x] Placeholder mappings for development
- [x] Comprehensive test coverage
- [x] Integration with HID system
- [x] Zone-based fallback system

### Phase 2 Success (Research Tools) ⏸️ PENDING

- [ ] HID traffic analysis tools
- [ ] Systematic matrix testing
- [ ] Protocol pattern recognition
- [ ] Research documentation

### Phase 3 Success (Accurate Mappings) ⏸️ PENDING

- [ ] Accurate key addresses for at least one Apex Pro model
- [ ] Validated per-key RGB command format
- [ ] Error handling for invalid addresses
- [ ] Production-ready key mapping database

### Phase 4 Success (Full Implementation) ⏸️ PENDING

- [ ] Per-key RGB control in CLI
- [ ] Advanced lighting effects (ripple, reactive, etc.)
- [ ] GameSense per-key integration
- [ ] Support for multiple Apex Pro variants
- [ ] ANSI and ISO layout support

## Conclusion

**Current Status**: Key mapping infrastructure is complete with placeholder data. The `ZoneFallback` system provides a working approximation of per-key effects using zone-based control while the actual HID per-key protocol remains unknown.

**Critical Path**: Hardware research is the blocking factor. All software infrastructure — `KeyMapping`, `PerKeyRgbBuilder`, `PerKeyRgbCommand`, `ZoneFallback` — is ready to support per-key control once the protocol is reverse-engineered.

**Risk Assessment**: High dependency on hardware access and reverse engineering success. The zone-based fallback ensures the project remains usable while research continues.

**Next Actions**:
1. Capture USB traffic with SteelSeries GG (WINE/Lutris) to identify per-key command format
2. Run focused bulk tests in the 0x10–0x1F and 0x28–0x2C ranges
3. Analyze traffic patterns from community projects (apex-tux, msi-perkeyrgb)
4. Prioritize hardware acquisition for research

---

**Document Status**: Complete for current development phase
**Last Updated**: 2026-03-01
**Next Review**: When hardware access is available or new protocol information is discovered
# SteelSeries Device Registry

Generated: 2026-05-26  
Source: `src/devices/mod.rs` (ssgg), cross-referenced with connected hardware.

---

## Currently Registered in ssgg

### Keyboards

| Constant | PID | Name | Zones | Per-Key | Actuation | Status |
|----------|-----|------|-------|---------|-----------|--------|
| `APEX_PRO` | 0x1610 | Apex Pro | 1 | No | ? | Unverified |
| `APEX_PRO_TKL` | 0x1614 | Apex Pro TKL | 1 | No | ? | Unverified |
| `APEX_PRO_TKL_2023` | **0x1628** | Apex Pro TKL (2023) | 9 | ? | ? | **Connected** ✅ |
| `APEX_PRO_TKL_2023_WIRELESS` | 0x1632 | Apex Pro TKL (2023) Wireless | 9 | ? | ? | Unverified |
| `APEX_PRO_TKL_2023_WIRELESS_2` | 0x1630 | Apex Pro TKL (2023) Wireless | 9 | ? | ? | Unverified |
| `APEX_3` | 0x161A | Apex 3 | 10 | No | No | Unverified |
| `APEX_3_TKL` | 0x1622 | Apex 3 TKL | 9 | No | No | Unverified |
| `APEX_5` | 0x161C | Apex 5 | 1 | No | No | Unverified |
| `APEX_7` | 0x1612 | Apex 7 | 1 | No | No | Unverified |
| `APEX_7_TKL` | 0x1616 | Apex 7 TKL | 1 | No | No | Unverified |

**Zone count note**: `APEX_PRO`, `APEX_PRO_TKL`, `APEX_5`, `APEX_7`, `APEX_7_TKL` all return `1` 
zone in `zone_count_for_product_id()`. These devices likely have multiple zones — pending 
verification from GG.Models.dll decompilation or USBPcap captures.

### Headsets

| Constant | PID | Name | Audio | RGB | Status |
|----------|-----|------|-------|-----|--------|
| `ARCTIS_1` | 0x12AD | Arctis 1 / Arctis 7 (2017) | Yes | No | Unverified |
| `ARCTIS_1_WIRELESS` | 0x12B3 | Arctis 1 Wireless | Yes | No | Unverified |
| `ARCTIS_5` | 0x12AA | Arctis 5 | Yes | Yes | Unverified |
| `ARCTIS_7_2019` | 0x12CF | Arctis 7 (2019) | Yes | No | Unverified |
| `ARCTIS_9` | 0x12C2 | Arctis 9 | Yes | No | Unverified |
| `ARCTIS_PRO` | 0x1252 | Arctis Pro | Yes | Yes | Unverified |
| `ARCTIS_PRO_WIRELESS` | 0x1290 | Arctis Pro Wireless | Yes | Yes | Unverified |
| `ARCTIS_NOVA_PRO` | 0x12E0 | Arctis Nova Pro | Yes | No | Unverified |
| `ARCTIS_NOVA_PRO_WIRELESS` | 0x12E4 | Arctis Nova Pro Wireless | Yes | No | Unverified |
| `ARCTIS_NOVA_5` | 0x12EA | Arctis Nova 5 | Yes | No | Unverified |
| `ARCTIS_NOVA_3` | 0x12EC | Arctis Nova 3 | Yes | No | Unverified |
| `ARCTIS_NOVA_1` | 0x12EE | Arctis Nova 1 | Yes | No | Unverified |
| `ARCTIS_NOVA_PRO_OMNI` | 0x2290 | Arctis Nova Pro Omni | Yes | No | Unverified |

---

## Connected Hardware (this session)

```
VID: 0x1038
PID: 0x1628 → APEX_PRO_TKL_2023 (✅ registered, name confirmed by Windows)
```

**HID interfaces**:
- MI_00: Standard keyboard (keystrokes)
- MI_01: Vendor-defined (candidate: RGB/control)
- MI_02: Control interface (candidate: RGB/control)
- MI_03: Consumer control (media keys)
- MI_04: Vendor-defined (candidate: RGB/control, no COL sub-interfaces → dedicated)

**RE target for Step 2**: Run Wireshark + USBPcap, filter on MI_04, trigger RGB changes in GG.

---

## Key Mapping Validation (Apex Pro TKL 2023)

**Source**: Prism `zone_cache` table, device_id=242, 2026-05-26.

`src/devices/key_mapping.rs` TKL 2023 mapping is **largely correct**. One discrepancy found:

| Code | ssgg mapping | Prism zone_cache | Status |
|------|-------------|-----------------|--------|
| HID 101 | `Menu` | **Absent** | ⚠️ ssgg maps a key that doesn't exist on this keyboard |
| HID 240 | `SteelSeriesKey` | FN key (confirmed) | ✅ Correct |
| HID 50 | In `tkl_hid_codes` vec | **Absent** | Non-US ISO key, not on US layout |
| HID 100, 133, 135-139 | In `tkl_hid_codes` vec | **Absent** | International layout keys only |

Prism confirms 84 zones for the US Apex Pro TKL 2023 (not 87 as in migration files, which cover international variants).

The `zone_count_for_product_id()` returning 9 for `APEX_PRO_TKL_2023` refers to **lighting zones** (logical RGB groups), not total keys. The device has 84 individually addressable keys plus 9 zone groups.

See `docs/development/prism-schema.md` for the full 84-key zone table with HID codes, coordinates, and actuation levels.

## Open Questions

- [ ] Which keyboards in the list actually support per-key RGB? (need GG.Models.dll)
- [ ] What are the correct zone counts for APEX_PRO, APEX_PRO_TKL, APEX_5, APEX_7, APEX_7_TKL?
- [ ] Do wireless Arctis models use the same HID protocol over the USB dongle?
- [ ] Does PID 0x1628 (Apex Pro TKL 2023) use command 0x40 or 0x23 for per-key RGB?
- [ ] Should `Menu (HID 101)` be removed from the TKL 2023 key mapping? (Prism says it's not on the US keyboard)

---

## Missing PIDs (from community reports / GitHub issues)

These PIDs appear in GitHub issues but are not yet in ssgg:

| PID | Device | Source | Priority |
|-----|--------|--------|----------|
| (none known yet) | — | — | — |

_Update this table after GG.Models.dll decompilation (Step 1a of RE plan)._

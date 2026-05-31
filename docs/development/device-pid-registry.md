# SteelSeries Complete VID/PID Registry

Generated: 2026-05-26  
Source: `C:\Program Files\SteelSeries\GG\apps\engine\firmware\*/version.json`  
Method: Firmware folder IDs encode `(VID << 16) | PID`; verified against confirmed device (apex_pro_tkl_2022 = folder 272111144 = 0x10381628 ✓)

All devices use VID `0x1038` unless noted.

---

## Keyboards (Apex Series)

| PID | GG firmware name | ssgg const | Status |
|-----|-----------------|-----------|--------|
| 0x1200 | apex-raw | — | Missing |
| 0x1202 | apex | — | Missing |
| 0x1206 | apex-350 | — | Missing |
| 0x1208 | apex-300 | — | Missing |
| 0x1600 | apex_m800 | — | Missing |
| 0x1605 | dota2_apex_m800 | — | Missing |
| 0x1607 | apex_m500 | — | Missing |
| 0x160C | apex_m400 | — | Missing |
| 0x160E | apex_100 | — | Missing |
| 0x1610 | apex_pro | `APEX_PRO` | ✅ |
| 0x1612 | apex_7 | `APEX_7` | ✅ |
| 0x1614 | apex_pro_tkl | `APEX_PRO_TKL` | ✅ |
| 0x1616 | apex_150 | `APEX_7_TKL` | ❌ **BUG: ssgg uses wrong PID for APEX_7_TKL** |
| 0x1618 | apex_7_tkl | — | ❌ **BUG: APEX_7_TKL should use 0x1618, not 0x1616** |
| 0x161A | apex_3 | `APEX_3` | ✅ |
| 0x161C | apex_5 | `APEX_5` | ✅ |
| 0x161E | apex_pro_mini | — | Missing |
| 0x1620 | apex_9_mini | — | Missing |
| 0x1622 | apex_3_tkl | `APEX_3_TKL` | ✅ |
| 0x1624 | apex_pro_mini_wireless_dongle | — | Missing |
| 0x1626 | apex_pro_mini_wireless | — | Missing |
| 0x1628 | apex_pro_tkl_2022 | `APEX_PRO_TKL_2023` | ✅ (GG calls it "2022") |
| 0x1630 | apex_pro_tkl_wireless_dongle | `APEX_PRO_TKL_2023_WIRELESS_2` | ✅ |
| 0x1632 | apex_pro_tkl_wireless | `APEX_PRO_TKL_2023_WIRELESS` | ✅ |
| 0x1634 | apex_9_tkl | — | Missing |
| 0x1640 | apex_pro_2024 | — | Missing |
| 0x1642 | apex_pro_tkl_2024 | — | Missing |
| 0x1644 | apex_pro_tkl_wireless_2024_dongle | — | Missing |
| 0x1646 | apex_pro_tkl_wireless_2024 | — | Missing |
| 0x1648 | apex-pro-mini-2024 | — | Missing |
| 0x1650 | apex_5_2024 | — | Missing |
| 0x1652 | apex_7_2024 | — | Missing |

**Note on apex_m750**: folder IDs 272107030/272107031 (VID 0x1038, but upper bits don't match 0x1038). These may use different VID ranges or the VID encoding differs for older devices.

---

## Headsets (Arctis Series)

| PID | GG firmware name | ssgg const | Status |
|-----|-----------------|-----------|--------|
| 0x1240 | siberia_840 | — | Missing |
| 0x1250 | arctis_5 (2017) | — | Missing |
| 0x1252 | arctis_pro | `ARCTIS_PRO` | ✅ |
| 0x1260 | arctis_7_dongle | — | Missing |
| 0x1261 | arctis_7 (2017) | — | Missing |
| 0x1290 | arctis_pro_wireless | `ARCTIS_PRO_WIRELESS` | ✅ |
| 0x1292 | arctis_pro_wireless_headset | — | Missing |
| 0x12AA | arctis_5_2018 | `ARCTIS_5` | ✅ |
| 0x12AD | arctis_7_2018_tx | `ARCTIS_1` | ❌ **BUG: ssgg const name wrong (this is Arctis 7 2018 TX dongle)** |
| 0x12AE | arctis_7_2018_rx | — | Missing |
| 0x12B1 | arctis_9x | — | Missing |
| 0x12B3 | arctis_1w_tx | `ARCTIS_1_WIRELESS` | ✅ |
| 0x12B4 | arctis_1w_rx | — | Missing |
| 0x12B6 | arctis_1x_tx | — | Missing |
| 0x12B7 | arctis_1x_rx | — | Missing |
| 0x12C0 | arctis_9_rx | — | Missing |
| 0x12C2 | arctis_9_tx | `ARCTIS_9` | ✅ |
| 0x12CB | arctis_nova_pro | — | ❌ **Missing (ssgg has wrong PID for this)** |
| 0x12CD | arctis_nova_pro_xbox | — | Missing |
| 0x12CF | (unknown) | `ARCTIS_7_2019` | ? Not in GG firmware list |
| 0x12D5 | arctis_7p_tx | — | Missing |
| 0x12D6 | arctis_7p_rx | — | Missing |
| 0x12D7 | arctis-7x-tx | — | Missing |
| 0x12D8 | arctis-7x-rx | — | Missing |
| 0x12E0 | arctis_nova_pro_wireless_tx | `ARCTIS_NOVA_PRO` | ❌ **BUG: ssgg labels TX dongle as "Nova Pro" (wired)** |
| 0x12E2 | arctis_nova_pro_wireless_rx | — | Missing |
| 0x12E4 | (unknown) | `ARCTIS_NOVA_PRO_WIRELESS` | ? Not in GG firmware list |
| 0x12E5 | arctis_nova_pro_wireless_xbox_tx | — | Missing |
| 0x12E8 | arctis_nova_pro_wireless_xbox_rx | — | Missing |
| 0x12EA | (unknown) | `ARCTIS_NOVA_5` | ? Not in GG firmware list (Nova 5 = 0x2230/0x2232) |
| 0x12EC | arctis_nova_3 | `ARCTIS_NOVA_3` | ✅ |
| 0x12EE | (unknown) | `ARCTIS_NOVA_1` | ? Not in GG firmware list |
| 0x12F0 | arctis_nova_4_rx | — | Missing |
| 0x12F2 | arctis_nova_4_tx | — | Missing |
| 0x12F4 | arctis_nova_4x_rx | — | Missing |
| 0x12F6 | arctis_nova_4x_tx | — | Missing |
| 0x12FA | arctis_nova_pro_v2 | — | Missing |
| 0x2200 | arctis_nova_7_rx | — | Missing |
| 0x2202 | arctis_nova_7_tx | — | Missing |
| 0x2204 | arctis_nova_7x_rx | — | Missing |
| 0x2206 | arctis_nova_7x_tx | — | Missing |
| 0x2208 | arctis_nova_7p_rx | — | Missing |
| 0x220A | arctis_nova_7p_tx | — | Missing |
| 0x220C | arctis_7_plus_rx | — | Missing |
| 0x220E | arctis_7_plus_tx | — | Missing |
| 0x2210 | arctis_7p_plus_rx | — | Missing |
| 0x2212 | arctis_7p_plus_tx | — | Missing |
| 0x2214 | arctis_7x_plus_rx | — | Missing |
| 0x2216 | arctis_7x_plus_tx | — | Missing |
| 0x2230 | arctis_nova_5_rx | — | Missing (ssgg's `ARCTIS_NOVA_5 = 0x12EA` is probably wrong) |
| 0x2232 | arctis_nova_5_tx | — | Missing |
| 0x2238 | arctis_nova_7_diablo_iv_rx | — | Missing |
| 0x2240 | arctis_nova_pro_wireless_v2_rx | — | Missing |
| 0x2244 | arctis_nova_elite_tx | — | Missing |
| 0x2249 | arctis_nova_elite_rx | — | Missing |
| 0x2251 | arctis_nova_5x_rx | — | Missing |
| 0x2253 | arctis_nova_5x_tx | — | Missing |
| 0x2267 | arctis_nova_3_wireless_rx | — | Missing |
| 0x2269 | arctis_nova_3_wireless_tx | — | Missing |
| 0x2288 | arctis_nova_7p_gen2_rx | — | Missing |
| 0x2290 | arctis_nova_pro_omni_tx | `ARCTIS_NOVA_PRO_OMNI` | ✅ |
| 0x2296 | arctis_nova_pro_omni_rx | — | Missing |
| 0x2298 | arctis_nova_7p_gen2_tx | — | Missing |
| 0x227C | arctis_nova_7_gen2_rx | — | Missing |
| 0x227E | arctis_nova_7_gen2_tx | — | Missing |
| 0x229C | arctis_nova_7x_gen2_rx | — | Missing |
| 0x229E | arctis_nova_7x_gen2_tx | — | Missing |
| 0x230A | arctis_gamebuds_dongle | — | Missing |
| 0x230C | arctis_gamebuds_case | — | Missing |

---

## Mice (Rival/Sensei/Aerox/Prime Series)

Not currently in ssgg scope. PIDs documented for completeness:

| PID | GG firmware name |
|-----|-----------------|
| 0x0472 | rival_150 |
| 0x0475 | rival_160 |
| 0x1361 | sensei |
| 0x1384 | rival |
| 0x1720 | rival_310 |
| 0x1724 | rival_600 |
| 0x1824 | rival_3 |
| 0x1836 | aerox_3 |
| 0x1850 | aerox_5 |
| 0x182E | prime |
| 0x182C | prime_plus |
| 0x1832 | sensei_ten |
| (full list in firmware dir) | ... |

---

## Speakers/Monitors (Arena)

| PID | GG firmware name |
|-----|-----------------|
| 0x1A00 | arena_7 |
| 0x1A05 | arena_9 |

---

## Microphones (Alias)

| PID | GG firmware name |
|-----|-----------------|
| 0x1B04 | alias |
| 0x1B06 | alias_pro |

---

## Confirmed Bugs in ssgg `product_ids` Module

### Bug 1: `APEX_7_TKL` has wrong PID (HIGH SEVERITY)

```rust
// WRONG:
pub const APEX_7_TKL: u16 = 0x1616;  // 0x1616 = apex_150 in GG, not apex_7_tkl

// CORRECT:
pub const APEX_7_TKL: u16 = 0x1618;  // 0x1618 = apex_7_tkl in GG firmware
// Also add:
pub const APEX_150: u16 = 0x1616;    // apex_150 (different product)
```

### Bug 2: `ARCTIS_1` PID maps to Arctis 7 2018 TX dongle

```rust
// CURRENT (misleading):
pub const ARCTIS_1: u16 = 0x12AD;  // GG: arctis_7_2018_tx (TX dongle, not Arctis 1)
```
The comment in ssgg says "Note: ARCTIS_7 (2017) also uses this ID" — this is partially correct.
0x12AD is the Arctis 7 2018 TX dongle. The actual Arctis 1 PIDs may be different.

### Bug 3: `ARCTIS_NOVA_PRO` maps to the Nova Pro Wireless TX dongle

```rust
// WRONG label:
pub const ARCTIS_NOVA_PRO: u16 = 0x12E0;  // GG: arctis_nova_pro_wireless_tx
// Missing:
// pub const ARCTIS_NOVA_PRO: u16 = 0x12CB;  // GG: arctis_nova_pro (wired)
```

### Bug 4: PIDs without GG firmware evidence

```rust
pub const ARCTIS_NOVA_PRO_WIRELESS: u16 = 0x12E4;  // Not in GG firmware
pub const ARCTIS_NOVA_5: u16 = 0x12EA;              // Not in GG firmware (Nova 5 = 0x2230/0x2232)
pub const ARCTIS_NOVA_1: u16 = 0x12EE;              // Not in GG firmware
```
These may be community-discovered or outdated. The Arctis Nova 5 has separate RX (0x2230) and TX (0x2232) PIDs.

---

## Prioritized ssgg Updates

1. **Fix APEX_7_TKL PID** (0x1616 → 0x1618) — correctness bug affecting all Apex 7 TKL users
2. **Add new Apex Pro 2024 variants** (0x1640-0x1652) — new hardware support
3. **Add Apex Pro Mini variants** (0x161E, 0x1620, 0x1624, 0x1626) — common devices
4. **Add Arctis Nova 7 series** (0x2200-0x220A) — popular current headset line
5. **Add Arctis Nova 5 correct PIDs** (0x2230, 0x2232) — fix existing wrong entry
6. **Add Arctis Nova 3 Wireless** (0x2267, 0x2269)
7. **Investigate ARCTIS_7_2019 = 0x12CF** — not in GG firmware, may be incorrect

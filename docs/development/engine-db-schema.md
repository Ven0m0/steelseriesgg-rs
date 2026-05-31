# SteelSeries Engine Main Database Schema

Generated: 2026-05-26  
Source: `C:\ProgramData\SteelSeries\GG\apps\engine\db\database.db` (SQLite, ~44 MB)  
Device: Apex Pro TKL 2023 (apex_pro_tkl_2022 in GG, PID 0x1628, engine devices.id = 242)

---

## Database Role

This is the primary engine database. It stores:
- **Device records** — one row per supported SteelSeries device model
- **Configurations** — named per-device profiles (Default, Gaming, etc.)
- **Hall effect actuation settings** — per-key actuation point, release point, rapid trigger
- **Key remappings** — per-key function codes and key codes
- **OLED display bitmaps** — raw 128×64 1-bpp frame data
- **Macro bindings, SSE commands, GameSense game integrations**

ssgg is primarily concerned with the `devices` and `configurations` tables.

---

## `devices` — Device model registry

```sql
CREATE TABLE "devices" (
    id                          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT UNIQUE,
    product_id                  INTEGER NOT NULL,  -- (VID << 16) | PID folder encoding
    name                        TEXT NOT NULL UNIQUE,
    full_name                   TEXT NOT NULL,
    type                        INTEGER NOT NULL,
    settings                    TEXT NOT NULL DEFAULT '{}',          -- JSON, current deployed settings
    undeployedsettings          TEXT NOT NULL DEFAULT '{}',
    model_type_id               INT NOT NULL DEFAULT 0,
    hidden                      INT NOT NULL DEFAULT 0,
    global_device_settings      TEXT NOT NULL DEFAULT ''
)
```

**`product_id` encoding**: `(VID << 16) | PID` — same as the firmware folder name.  
- Apex Pro TKL 2023: `product_id = 272111144` = `0x10381628` → VID=0x1038, PID=0x1628 ✓  
- This is **not** the 16-bit USB PID; it is the 32-bit composite.

**`type` values** (observed):
- 0 = keyboard
- 1 = mouse
- 3 = headset
- 4 = gamepad
- 5 = misc (Tobii, USB hub)
- 6 = virtual/software device
- 7 = mousepad
- 8 = motherboard
- 10 = laptop keyboard (MSI ALC/KLC)
- 11 = speakers (Arena)
- 12 = microphone (Alias)

---

## `devices.settings` JSON — Apex Pro TKL 2023

The `settings` column holds the currently-deployed device state. Top-level keys:

| Key | Type | Description |
|-----|------|-------------|
| `name` | `{characters: [u8; 24]}` | Device name as ASCII byte array, null-padded |
| `region` | `{region_id: i32}` | Layout region ID (-1 = default/US) |
| `meta_toggle_hid` | `{no_live_deploy: 1, code: 240}` | FN key HID code (240 = SteelSeries key) |
| `mappings` | `{no_live_deploy: 1, button_mappings: {buttons: [...]}}` | Per-key function mappings |
| `hall_thresholds` | `{keys: [{hid, l, h}]}` | Per-key actuation thresholds |
| `second_actuation_thresholds` | `{keys: [{hid, l, h}]}` | Per-key rapid trigger thresholds |
| `oled_display` | `{data: [u8; 1024]}` | OLED 128×64 1-bpp bitmap (row major) |
| `apex_2022_oled_display_sequence` | object | OLED animation sequence |

### `hall_thresholds` — Actuation control

Per-key actuation hysteresis pair:
```json
{
  "keys": [
    {"hid": 4, "l": 46, "h": 50},
    ...
  ]
}
```

- `hid`: USB HID Usage ID (4 = A, 5 = B, ..., 240 = SteelSeriesKey/FN)
- `l`: lower threshold (units: scaled 0–100, 50 ≈ 2.0 mm)
- `h`: upper threshold (units: scaled 0–100, 50 ≈ 2.0 mm)

**Unit calibration**: GG's default actuation is 2.0mm, and the default `h=50` confirms **1 unit ≈ 0.04mm** (25 units/mm, or 1/100th of the 4mm OmniPoint travel). The hysteresis window `h - l = 4` ≈ 0.16mm.

**Default values**: `l=46, h=50` (actuation at 2.0mm, release at 1.84mm)  
**Gaming profile**: `l=5, h=5` (actuation at 0.2mm, no hysteresis — rapid trigger)

### `second_actuation_thresholds` — Rapid trigger

Same structure. When `l=255, h=255`, rapid trigger is **disabled** for that key.  
When `l=0, h=0`, rapid trigger is **enabled** at minimum (any upward movement re-triggers).

### `mappings.button_mappings.buttons[]`

```json
{"hid_code": 4, "function": 81, "key_codes": [4, 0, 0, 0]}
```
- `hid_code`: key being remapped
- `function`: 81 = passthrough (standard keypress), other values = macro/media/custom
- `key_codes`: output keycodes (4 = HID Usage A when function=81)

### `oled_display.data`

1024 bytes = 128×64 pixels at 1 bpp, row-major order. The default value shows the SteelSeries logo.  
All-zeros = blank display.

---

## `configurations` — Named profiles

```sql
CREATE TABLE configurations (
  id              UUID PRIMARY KEY UNIQUE,
  device_id       INTEGER NOT NULL,        -- references devices.id
  name            TEXT,                    -- "Default", "Gaming", "Fortnite", etc.
  settings        TEXT,                    -- JSON, same structure as devices.settings
  active          INTEGER DEFAULT 0,       -- 1 = currently active profile
  user_id         INTEGER NOT NULL,
  mutable         INTEGER DEFAULT 1,
  last_deployed   INTEGER DEFAULT 0,
  undeployedsettings TEXT DEFAULT '{}',
  cloud_last_updated_at TEXT DEFAULT '',
  is_deleted      INTEGER DEFAULT 0,
  sync_config     INTEGER DEFAULT 0
)
```

Observed profiles for Apex Pro TKL 2023 (`device_id=242`):
- Default (×4 — multiple copies), Gaming (active), main, Config 4, Config 1 2, Fortnite

### `configurations.settings` — Additional keys vs devices.settings

The config settings extend the base with rapid trigger fields:

| Key | Type | Description |
|-----|------|-------------|
| `rapid_tap_enable` | `{enabled: 0\|1}` | Global rapid tap on/off |
| `rapid_tap_pairs` | object | Rapid tap key pair config |
| `rapid_trigger_sensitivity` | `{keys: [{hid, sensitivity}]}` | Per-key RT sensitivity (1=normal, 2=reduced) |
| `release_mode` | `{keys: [{hid, mode}]}` | Per-key release behavior |
| `guid` | string | Config unique identifier |

### `release_mode.mode` values

- `2` = standard release (deactuates at l threshold going up)
- `4` = modifier/instant release (Ctrl, Shift, Alt, Space, Enter — no travel-based release)

Observed mode-4 keys in Gaming config: hid 40 (F7), 42 (F9), 43 (F10), 44 (F11), 57 (CapsLock), 226 (LAlt), 227 (RGui), 230 (RAlt), 231 (RCtrl), 240 (FN).

---

## `physical_devices` — Currently connected hardware

```sql
CREATE TABLE physical_devices (
  product_id              INTEGER NOT NULL,  -- raw 16-bit USB PID (not composite)
  name                    TEXT,
  full_name               TEXT,
  firmware_version        TEXT,
  fw_incompatible         INTEGER DEFAULT 0,
  fw_update_required      TEXT DEFAULT 0,
  cached_main_device_pid  INT DEFAULT 0,
  is_exclusive            INT DEFAULT 0,
  cached_type_id          INT DEFAULT 0
)
```

Populated only when a device is physically connected. Empty when device is disconnected.

---

## `wireless_device_information`

```sql
CREATE TABLE wireless_device_information (
  transmitter_product_id INTEGER NOT NULL PRIMARY KEY,
  transmitter_name       TEXT DEFAULT '',
  receiver_product_id    INTEGER DEFAULT 0,
  receiver_id            INTEGER DEFAULT 0,
  receiver_name          TEXT DEFAULT ''
)
```

Maps TX dongle PID → RX headset PID for wireless pairs.

---

## ssgg Actuation Control Gap Analysis

The `src/bin/discover_actuation.rs` binary probes for the HID command that sends actuation thresholds. Based on this DB analysis, the data to send is:

```
For each key:
  hid_code: u8    (USB HID Usage ID)
  l: u8           (actuation threshold, 0–100 scale, ~25 units/mm)
  h: u8           (release threshold, same scale)
```

**Hypothesis for HID command `0x2D` (ActuationControl)**:
```
[CommandCode=0x2D, key_count: u8, keys: [(hid: u8, l: u8, h: u8) × key_count]]
```

This is unconfirmed without USB HID captures. The `discover_actuation.rs` binary should iterate command codes and observe which triggers a device response / LED flash.

### Rapid trigger fields

Rapid trigger requires sending:
1. `hall_thresholds` per-key (actuation depth)
2. `second_actuation_thresholds` per-key (255/255 = disabled, 0/0 = enabled at minimum)
3. `rapid_trigger_sensitivity` per-key (1 or 2)
4. `release_mode` per-key (2 = travel, 4 = instant)

These are four separate HID commands or combined into one. The Apex Pro TKL 2023 rapid trigger feature requires all four parameters.

---

## Other Notable Tables

| Table | Content |
|-------|---------|
| `game_integration_games` | Known games by name |
| `game_integration_registered_events` | Per-game events (e.g., "HEALTH", "AMMO") |
| `game_integration_event_bindings` | Event → key binding (GameSense config) |
| `game_integration_presets` | Named GameSense presets |
| `macros` | Recorded macro sequences |
| `bindings` | Macro trigger → config bindings (via config_id UUID) |
| `sse_commands` | External process commands (path + params) |
| `sub_apps` | Registered GG sub-app records |
| `universal_triggers` | Universal trigger definitions |
| `applications` | Registered applications for GG library |
| `loadout_presets` | Multi-device loadout configurations |

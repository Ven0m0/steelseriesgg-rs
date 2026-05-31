# Pre-flight RE Findings

Generated: 2026-05-26  
Method: DLL size analysis + PowerShell enumeration + SQLite binary extraction

---

## Decompilability Status: CONFIRMED ✅

GG's core assemblies are standard managed .NET IL — not AOT compiled, not trimmed.
ILSpy/dnSpy will produce near-source-quality C#.

| DLL | Size (KB) | Type | ILSpy? |
|-----|-----------|------|--------|
| GG.API.dll | 51.6 | Managed | ✅ |
| GG.Services.dll | 172.6 | Managed | ✅ |
| GG.Models.dll | 81.1 | Managed | ✅ |
| GG.Database.dll | 32.1 | Managed | ✅ |
| GG.Interop.Services.dll | 121.6 | Managed | ✅ |
| SteelSeriesGGEZ.dll | 76.6 | Managed | ✅ |
| InputLib.dll | 166.6 | **Native Win32** | Ghidra only |
| SteelSeriesGameOverlay.dll | 394.1 | Native | Ghidra only |

Confidence: high. If any DLL fails to decompile, try de4dot first (IL obfuscation stripper),
then fall back to Ghidra + x64dbg.

---

## Connected Device (Step 1b Result)

Device on this machine:

| Field | Value |
|-------|-------|
| Name | SteelSeries Apex Pro TKL |
| VID | 0x1038 |
| PID | **0x1628** |
| ssgg name | `APEX_PRO_TKL_2023` |
| ssgg status | ✅ Already registered in `src/devices/mod.rs:337` |

**HID Interfaces exposed** (all from the same composite USB device):

| Interface | Windows name | Purpose |
|-----------|-------------|---------|
| MI_00 | SteelSeries Apex Pro TKL (keyboard) | Standard HID keyboard |
| MI_00 COL01 | HID-compliant mouse | (not used for RGB) |
| MI_00 COL02 | HID Keyboard Device | Standard key input |
| MI_00 COL03 | HID-compliant consumer control | Media keys |
| MI_01 | HID-compliant vendor-defined device | **Likely RGB/control interface** |
| MI_02 | SteelSeries Apex Pro TKL (control) | **Likely RGB/control interface** |
| MI_02 COL01 | HID-compliant mouse | (not used for RGB) |
| MI_02 COL02 | HID Keyboard Device | Standard key input |
| MI_02 COL03 | HID-compliant consumer control | Media keys |
| MI_03 | USB Input Device | Input/macro |
| MI_04 | HID-compliant vendor-defined device | **Likely RGB/actuation control** |

**Key question for Wireshark**: Which MI_0x interface carries the RGB HID reports?
When running USBPcap, filter by interface number and check which one receives
`SET_REPORT` / `INTERRUPT OUT` transfers when changing RGB in GG.

Hypothesis: **MI_04** (vendor-defined, no-COL children) is the dedicated control interface.
Validate by filtering Wireshark on endpoint associated with MI_04.

---

## SQLite Databases

| Path | Size | Purpose |
|------|------|---------|
| `C:\ProgramData\SteelSeries\GG\db\database.db` | 628 KB | Main GG: GameSense, users, loadouts, settings |
| `C:\ProgramData\SteelSeries\GG\db\ggez.db` | 60 KB | GG EZ edition data |
| `C:\ProgramData\SteelSeries\GG\apps\engine\db\database.db` | 43,132 KB | Engine: device history, full config |
| `C:\ProgramData\SteelSeries\GG\apps\engine\prism\db\database.db` | 2,692 KB | **Prism: RGB profiles, effect configs** |
| `C:\ProgramData\SteelSeries\GG\apps\sonar\db\database.db` | 2,604 KB | **Sonar: audio EQ, routing, presets** |
| `C:\ProgramData\SteelSeries\GG\apps\moments\db\database.db` | 88 KB | Moments: video capture metadata |

**Note**: All databases are locked while GG is running. Copy before reading:
```powershell
Copy-Item "C:\ProgramData\SteelSeries\GG\db\database.db" "$env:TEMP\gg-db.db"
```

---

## Main GG Database Schema (Extracted)

### GameSense Tables

**`game_integration_games`** — registered games
```sql
CREATE TABLE game_integration_games (
    id UUID PRIMARY KEY UNIQUE,
    game_name TEXT NOT NULL DEFAULT '',
    game_display_name TEXT NOT NULL DEFAULT '',
    user_visible INTEGER NOT NULL DEFAULT 1,
    enabled int NOT NULL DEFAULT 1,
    reserved INTEGER NOT NULL DEFAULT 0,
    deinitialize_timer_length int NOT NULL DEFAULT 15000,  -- default: 15s
    developer TEXT NOT NULL DEFAULT '',
    launchable_engine_app_id UUID NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 0,
    app_logo_url TEXT NOT NULL DEFAULT '',
    app_tile_url TEXT NOT NULL DEFAULT '',
    local_app_logo_path TEXT NOT NULL DEFAULT '',
    local_app_tile_path TEXT NOT NULL DEFAULT '',
    autoclip_helper_text TEXT NOT NULL DEFAULT '',
    options nvarchar(255) NOT NULL DEFAULT '{}'
)
```

**ssgg gap**: ssgg's `GameMetadata` struct is missing: `user_visible`, `enabled`, `version`, `app_logo_url`, `app_tile_url`, `options`.
Most of these are UI-only; `options` may be worth investigating.

**`game_integration_registered_events`** — events per game
```sql
CREATE TABLE game_integration_registered_events (
    id UUID PRIMARY KEY UNIQUE,
    game_id UUID NOT NULL DEFAULT '',
    event TEXT NOT NULL DEFAULT '',
    event_name_localization_key TEXT NOT NULL DEFAULT '',
    reserved INTEGER NOT NULL DEFAULT 0,
    user_visible INTEGER NOT NULL DEFAULT 1,
    icon_id INTEGER NOT NULL DEFAULT 0,
    min_value int NOT NULL DEFAULT 0,
    max_value int NOT NULL DEFAULT 100,
    is_fancy int NOT NULL DEFAULT 0,         -- complex event with data_fields?
    data_fields TEXT NOT NULL DEFAULT '[]',   -- JSON array of field definitions
    value_optional INTEGER NOT NULL DEFAULT 0
)
```

**ssgg gap**: ssgg's `EventBinding` struct has `min_value`, `max_value`, `icon_id`, `handlers` but
is missing `is_fancy`, `data_fields`, and `value_optional`. The `data_fields` field likely
enables complex event payloads beyond simple `value: i32`.

**`game_integration_event_bindings`** — handler bindings
```sql
CREATE TABLE game_integration_event_bindings (
    registered_event_id UUID NOT NULL DEFAULT '',
    level TEXT NOT NULL DEFAULT '',        -- priority layer?
    json TEXT NOT NULL DEFAULT '',         -- handler binding JSON (what ssgg's `handlers` field)
    user_id INTEGER NOT NULL DEFAULT 1,
    product_id INTEGER NOT NULL DEFAULT 0  -- target device (0 = all devices)
)
```

**ssgg gap**: ssgg's GameSense server doesn't track `product_id` per binding. GG can target
specific device classes (keyboard vs. headset). The `level` field is unknown — may be
user-defined binding layers (primary, secondary, game-override).

**`game_integration_presets`** — preset handler configs
```sql
CREATE TABLE game_integration_presets (
    id UUID PRIMARY KEY,
    preset TEXT NOT NULL   -- JSON handler configuration
)
```

**`game_integration_templates`** — event groups
```sql
CREATE TABLE game_integration_templates (
    id UUID PRIMARY KEY,
    game_id UUID NOT NULL REFERENCES game_integration_games(id) DEFAULT '',
    label TEXT NOT NULL DEFAULT '',
    event_names TEXT NOT NULL DEFAULT ''  -- comma-separated event names?
)
```

**`game_integration_templates_per_device`** — template → device mapping
```sql
CREATE TABLE game_integration_templates_per_device (
    template_id UUID NOT NULL REFERENCES game_integration_templates(id),
    product_id INTEGER NOT NULL
)
```

### Loadouts (GG's Profile System)

GG calls profiles "**Loadouts**" — not "Profiles".

**`loadouts`**
```sql
CREATE TABLE "loadouts" (
    id TEXT PRIMARY KEY UNIQUE NOT NULL,
    name TEXT UNIQUE NOT NULL,
    creation_time DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    last_deploy_time DATETIME NOT NULL DEFAULT '1970-01-01 00:00:00.000',
    last_manual_deploy_time DATETIME NOT NULL DEFAULT '1970-01-01 00:00:00.000'
)
```

**`loadouts_configs`** — what each loadout contains
```sql
CREATE TABLE loadouts_configs (
    loadout_id TEXT NOT NULL,
    config_type TEXT NOT NULL,      -- e.g., 'prism', 'sonar', 'engine2'
    config_metadata TEXT NOT NULL,  -- JSON metadata for the config
    config_id TEXT NOT NULL,        -- ID in the sub-app's own database
    UNIQUE (loadout_id, config_type, config_metadata),
    FOREIGN KEY(loadout_id) REFERENCES loadouts(id)
)
```

**`loadouts_bindings`** — input method triggers per loadout
```sql
CREATE TABLE loadouts_bindings (
    loadout_id TEXT NOT NULL,
    binding_type TEXT NOT NULL,   -- e.g., 'game', 'shortcut'
    binding_value TEXT NOT NULL,  -- e.g., game ID or key combo
    UNIQUE (binding_type, binding_value),
    FOREIGN KEY(loadout_id) REFERENCES loadouts(id)
)
```

**`loadouts_shortcut_bindings`** — keyboard shortcut assignments
```sql
CREATE TABLE loadouts_shortcut_bindings (
    shortcut_index INTEGER PRIMARY KEY UNIQUE NOT NULL,
    loadout_id TEXT NOT NULL DEFAULT ''
)
```

**Architecture insight**: Loadout RGB config lives in the **Prism database** (not the main GG DB).
The main GG DB only stores `config_id` → pointing to an entry in Prism's own DB.
Same for Sonar audio configs. This is a federated storage model.

### Settings

**`settings`** — global key-value config
```sql
CREATE TABLE settings (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
)
```

**`user_settings`** — per-user key-value config
```sql
CREATE TABLE user_settings (
    user_id INT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY(user_id, key)
)
```

**`system_settings`** — system-level config
```sql
CREATE TABLE system_settings (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
)
```

### Sub-app Registry

**`sub_apps`**
```sql
CREATE TABLE sub_apps (
    id INTEGER PRIMARY KEY AUTOINCREMENT UNIQUE,
    name TEXT NOT NULL UNIQUE,
    is_enabled BOOLEAN NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    is_mac_supported BOOLEAN NOT NULL DEFAULT 0,
    is_windows_supported BOOLEAN NOT NULL DEFAULT 0,
    executable_name TEXT NOT NULL DEFAULT '',
    toggle_via_settings BOOLEAN NOT NULL DEFAULT 0,
    auto_start BOOLEAN NOT NULL DEFAULT 0,
    is_browserview_supported BOOLEAN NOT NULL DEFAULT 0
)
```

Known sub-app names from data: `sonar`, `prism`, `engine2`, `moments`.

---

## ssgg Implementation Gaps (from schema analysis)

### GameSense API gaps

| Missing field | In GG table | Impact for ssgg |
|---------------|-------------|-----------------|
| `product_id` | `event_bindings` | ssgg can't target specific device classes per binding |
| `level` | `event_bindings` | Unknown — may be binding priority layer |
| `is_fancy` | `registered_events` | Complex event types with structured data |
| `data_fields` | `registered_events` | Beyond-scalar event data |
| `value_optional` | `registered_events` | Events that fire without a value |

### Profile system gaps

| GG concept | GG implementation | ssgg equivalent |
|------------|-------------------|-----------------|
| Loadout | `loadouts` table + configs | `Profile` TOML (subset) |
| Per-device RGB config | Prism DB (config_id) | `KeyboardProfile.effect` (simple) |
| Per-device audio config | Sonar DB (config_id) | `HeadsetProfile` (partial) |
| Loadout binding to game | `loadouts_bindings` | Not implemented |
| Shortcut switching | `loadouts_shortcut_bindings` | Not implemented |

### Missing from ssgg profiles

- Effect-specific parameters (speed, colors beyond one, fade delay)
- Per-zone effect overrides
- Loadout → game binding (auto-switch profile when game launches)
- Keyboard shortcut to switch profiles

---

## Next Manual Steps

1. **Read Prism DB** (stop GG first, or copy file):
   ```powershell
   Stop-Process -Name "SteelSeriesGGEZ" -Force
   sqlite3 "C:\ProgramData\SteelSeries\GG\apps\engine\prism\db\database.db" ".schema"
   ```
   → RGB effect schema, zone configs, color data format

2. **ILSpy: GG.API.dll** — extract controller routes and DTOs
3. **USBPcap capture** — connect keyboard, capture RGB changes while GG is running
   Filter: `usb.transfer_type == 0x01 && usb.idVendor == 0x1038`
   Target interface: MI_04 (vendor-defined) — hypothesis

---

## Artifacts Saved

- This file: `docs/development/preflight-findings.md`
- Capture directory: `docs/development/captures/` (empty, awaiting Wireshark captures)
- Decompile directory: `docs/development/decompiled/` (empty, awaiting ILSpy output)

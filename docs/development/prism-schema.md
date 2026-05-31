# SteelSeries GG Prism Database Schema

Generated: 2026-05-26  
Source: `C:\ProgramData\SteelSeries\GG\apps\engine\prism\db\database.db` (SQLite, ~2.7 MB)  
Device: Apex Pro TKL 2023 (VID 0x1038, PID 0x1628), device_id=242 in Prism

---

## Tables

### `configs` — RGB lighting configurations

```sql
CREATE TABLE configs (
    id                       TEXT     PRIMARY KEY,
    is_deleted               INTEGER  NOT NULL DEFAULT 0,
    is_hidden_immutable      INTEGER  NOT NULL DEFAULT 0,
    is_current               INTEGER  NOT NULL DEFAULT 0,
    is_last_manually_deployed INTEGER NOT NULL DEFAULT 0,
    name                     TEXT     NOT NULL DEFAULT '',
    config_json              TEXT     NOT NULL DEFAULT '',
    schema                   INTEGER  NOT NULL DEFAULT 0,
    is_per_device            INTEGER  NOT NULL DEFAULT 0
);
```

**Notes:**
- `config_json` stores the full RGB config for all three lighting states (base, idle, reactive) as a JSON blob.
- `schema` = 4 for user-created/preset-based configs; 0 for the immutable default.
- `is_per_device` = 1 when the config targets a specific device (via `config_per_device`).
- Active config determined by `is_current = 1`. On this device: id=`a06fc320-e0ab-4a33-a87c-846892e2546b`, name="Default".

**Example configs observed:**
- `Immutable Default` (schema=0, is_per_device=0) — built-in fallback
- `Default`, `Aqua`, `Chasing Ghosts`, `Comet`, `Freeway`, `Drain` (schema=4)

### `config_per_device` — per-device config bindings

```sql
CREATE TABLE config_per_device (
    config_id TEXT NOT NULL REFERENCES configs(id),
    device_id INTEGER NOT NULL
);
```

Maps configs with `is_per_device=1` to specific device IDs.

### `zone_cache` — per-key addressing table

```sql
CREATE TABLE zone_cache (
    device_id       INTEGER NOT NULL,
    unique_id       TEXT    NOT NULL,   -- 64-bit integer as TEXT; matches zoneIds in config_json
    x               INTEGER NOT NULL,   -- absolute x coordinate (bitmap space)
    y               INTEGER NOT NULL,   -- absolute y coordinate (bitmap space)
    bitmap_x        INTEGER,            -- normalized bitmap x (unused in queries observed)
    bitmap_y        INTEGER,            -- normalized bitmap y (unused in queries observed)
    hid_code        INTEGER NOT NULL,   -- USB HID Usage ID (page 7, Keyboard/Keypad)
    actuation_level INTEGER NOT NULL DEFAULT 0,  -- OmniPoint actuation point (600 = default ~1.0mm, 0 = not adjustable)
    linear_index    INTEGER,            -- sequential key index
    PRIMARY KEY (device_id, unique_id)
);
```

**Key insight**: The `unique_id` values in this table are the exact same integers used in `config_json`'s `"zoneIds"` arrays. This is the bridge between Prism's layout data and RGB config data.

**Actuation level semantics** (corrected from re-query):
- 0 = standard mechanical switch (23 keys — no OmniPoint)
- 100 = OmniPoint key at standard actuation (51 keys — main alphanumeric zone)
- 1000 = OmniPoint modifier keys or special keys (10 keys — e.g. Ctrl, Shift, etc.)

Note: these Prism values are **classification codes** for RGB effects (different reactive animation for mechanical vs OmniPoint), not the actual HID actuation threshold in mm. The actual per-key actuation thresholds are in the engine main DB (`engine/db/database.db`) `devices.settings.hall_thresholds`, where the unit is approximately 0.04mm (25 units/mm, range 0–100, default 50 ≈ 2.0mm).

### `device_cache` — device metadata

```sql
CREATE TABLE device_cache (
    device_id                INTEGER PRIMARY KEY,
    name                     TEXT,
    full_name                TEXT,
    device_type              INTEGER,
    whitelisted_lighting_type INTEGER,
    has_bitmap_coordinates   INTEGER,
    bitmap_x_min             INTEGER,
    bitmap_x_max             INTEGER,
    bitmap_y_min             INTEGER,
    bitmap_y_max             INTEGER,
    has_linear_indexing      INTEGER,
    linear_size              INTEGER
);
```

**Observed entry** for device_id=242:
```
name="apex_pro_tkl_2022"   (internal name — note "2022" even for the 2023 hardware)
full_name=(unknown)
has_bitmap_coordinates=1
bitmap_x_min=0, bitmap_x_max=17   (18 columns)
bitmap_y_min=0, bitmap_y_max=5    (6 rows)
has_linear_indexing=(unknown)
linear_size=(unknown)
```

### `presets` — built-in effect presets

```sql
CREATE TABLE presets (
    id                         TEXT PRIMARY KEY,
    mode                       TEXT,      -- "baseOrIdle"
    requires_bitmap_coordinates INTEGER,  -- 0 or 1
    name_locale_key            TEXT,      -- "bloom.presets.<effectName>"
    data                       TEXT       -- JSON effect config
);
```

27 presets observed, all with `mode="baseOrIdle"`. Effect names include:
- Without bitmap coords (0): steelseriesOrange, vaporDreams, westCoast, haze, prism, chasingGhosts, warpDrive, wabashAndLake, solar, staticFade, colorFusion, shavedIce, selfDestruct, clown, radioactive, redPulse, aqua, rainbowSplit, ...
- With bitmap coords (1): rainbow, discoMode (per-key gradient effects)

### `loadout_presets` — three-state presets

```sql
CREATE TABLE loadout_presets (
    id        TEXT PRIMARY KEY,
    name      TEXT,
    base_data TEXT,   -- JSON for the "base" lighting state
    idle_data TEXT    -- JSON for the "idle" lighting state
);
```

---

## Config JSON Structure

The `config_json` column encodes three independent lighting states:

```json
{
  "base": {
    "globalConfig": {
      "graphics": [
        {
          "type": 0,
          "zIndex": 0,
          "foreground": {
            "duration": 4690,
            "spatialWavelength": 64,
            "gradient": {
              "colors": [
                {"color": {"red": 255, "green": 0,   "blue": 255}, "position": 0},
                {"color": {"red": 255, "green": 234, "blue": 0},   "position": 85},
                {"color": {"red": 0,   "green": 204, "blue": 255}, "position": 170}
              ]
            }
          },
          "metadata": {"effectType": "COLOR_SHIFT"}
        }
      ],
      "defaultColor": {"red": 0, "green": 0, "blue": 0}
    },
    "deviceConfigs": [
      {
        "deviceId": 242,
        "zoneIds": ["68117837727006720", "..."],   // 84 entries for full keyboard
        "config": {
          "defaultColor": {"red": 255, "green": 0, "blue": 255},
          "metadata": {"effectType": "SINGLE_COLOR"}
        }
      }
    ]
  },
  "idle": { ... },
  "idleTimeMs": 0,
  "reactive": { ... }
}
```

**Effect types observed:**
- `"COLOR_SHIFT"` — animated gradient/spectrum cycle (`duration`, `spatialWavelength`, `gradient`)
- `"SINGLE_COLOR"` — static single color (`defaultColor`)
- `"BLOOM"` — reactive bloom/ripple effect (bitmap-coordinates required)

**Three lighting states:**
- `base` — active state (while in use)
- `idle` — activates after `idleTimeMs` ms of inactivity
- `reactive` — key-press reaction overlay (applied on top of base/idle)

This is a **critical gap vs. ssgg**: ssgg models one lighting state (`Effect`). GG has three independent states with its own transition timing.

---

## Complete Zone Map: Apex Pro TKL 2023 (device_id=242)

Sorted by keyboard row (y descending = top-to-bottom). `al` = actuation_level.

### Function Row (y=256) — standard switches (al=0)

| unique_id           | x   | hid | key        | al |
|---------------------|-----|-----|------------|----|
| 68117545676177408   | 140 |  41 | Escape     |  0 |
| 68117854913822720   | 212 |  58 | F1         |  0 |
| 68118005237678080   | 247 |  59 | F2         |  0 |
| 68118159856500736   | 283 |  60 | F3         |  0 |
| 68118314475323392   | 319 |  61 | F4         |  0 |
| 68118464799178752   | 354 |  62 | F5         |  0 |
| 68118619418001408   | 390 |  63 | F6         |  0 |
| 68118774036824064   | 426 |  64 | F7         |  0 |
| 68118924360679424   | 461 |  65 | F8         |  0 |
| 68119078979502080   | 497 |  66 | F9         |  0 |
| 68119229303357440   | 532 |  67 | F10        |  0 |
| 68119383922180096   | 568 |  68 | F11        |  0 |
| 68119538541002752   | 604 |  69 | F12        |  0 |

### Number Row (y=212)

| unique_id           | x   | hid | key        | al  |
|---------------------|-----|-----|------------|-----|
| 68117545673293824   | 140 |  53 | ` ~        | 600 |
| 68117700292116480   | 176 |  30 | 1 !        | 600 |
| 68117854910939136   | 212 |  31 | 2 @        | 600 |
| 68118005234794496   | 247 |  32 | 3 #        | 600 |
| 68118159853617152   | 283 |  33 | 4 $        | 600 |
| 68118314472439808   | 319 |  34 | 5 %        | 600 |
| 68118464796295168   | 354 |  35 | 6 ^        | 600 |
| 68118619415117824   | 390 |  36 | 7 &        | 600 |
| 68118774033940480   | 426 |  37 | 8 *        | 600 |
| 68118924357795840   | 461 |  38 | 9 (        | 600 |
| 68119078976618496   | 497 |  39 | 0 )        | 600 |
| 68119229300473856   | 532 |  45 | - _        | 600 |
| 68119383919296512   | 568 |  46 | = +        | 600 |
| 68119620142497792   | 623 |  42 | Backspace  | 600 |
| 68119907905306624   | 690 |  73 | Insert     |   0 |
| 68120058229161984   | 725 |  74 | Home       |   0 |
| 68120212847984640   | 761 |  75 | Page Up    |   0 |

### Tab Row / QWERTY Top (y=181)

| unique_id           | x   | hid | key        | al  |
|---------------------|-----|-----|------------|-----|
| 68117584325967872   | 149 |  43 | Tab        | 600 |
| 68117777599496192   | 194 |  20 | Q          | 600 |
| 68117932218318848   | 230 |  26 | W          | 600 |
| 68118082542174208   | 265 |   8 | E          | 600 |
| 68118237160996864   | 301 |  21 | R          | 600 |
| 68118391779819520   | 337 |  23 | T          | 600 |
| 68118542103674880   | 372 |  28 | Y          | 600 |
| 68118696722497536   | 408 |  24 | U          | 600 |
| 68118851341320192   | 444 |  12 | I          | 600 |
| 68119001665175552   | 479 |  18 | O          | 600 |
| 68119156283998208   | 515 |  19 | P          | 600 |
| 68119310902820864   | 551 |  47 | [ {        | 600 |
| 68119461226676224   | 586 |  48 | ] }        | 600 |
| 68119615845498880   | 622 |  49 | \ \|       | 600 |
| 68119817708961792   | 669 |  76 | Delete     |   0 |
| 68119972327784448   | 705 |  77 | End        |   0 |
| 68120122651639808   | 740 |  78 | Page Down  |   0 |

### Home Row (y=150)

| unique_id           | x   | hid | key        | al  |
|---------------------|-----|-----|------------|-----|
| 68117614388707328   | 156 |  57 | Caps Lock  | 600 |
| 68117837727006720   | 208 |   4 | A          | 600 |
| 68117992345829376   | 244 |  22 | S          | 600 |
| 68118142669684736   | 279 |   7 | D          | 600 |
| 68118297288507392   | 315 |   9 | F          | 600 |
| 68118451907330048   | 351 |  10 | G          | 600 |
| 68118602231185408   | 386 |  11 | H          | 600 |
| 68118756850008064   | 422 |  13 | J          | 600 |
| 68118911468830720   | 458 |  14 | K          | 600 |
| 68119061792686080   | 493 |  15 | L          | 600 |
| 68119216411508736   | 529 |  51 | ; :        | 600 |
| 68119366735364096   | 564 |  52 | ' "        | 600 |
| 68119615843467264   | 622 |  40 | Enter      | 600 |

### ZXCV Row (y=120)

| unique_id           | x   | hid | key        | al  |
|---------------------|-----|-----|------------|-----|
| 68117678811250688   | 171 | 225 | L Shift    | 600 |
| 68117962279092224   | 237 |  29 | Z          | 600 |
| 68118116897914880   | 273 |  27 | X          | 600 |
| 68118267221770240   | 308 |   6 | C          | 600 |
| 68118421840592896   | 344 |  25 | V          | 600 |
| 68118576459415552   | 380 |   5 | B          | 600 |
| 68118726783270912   | 415 |  17 | N          | 600 |
| 68118881402093568   | 451 |  16 | M          | 600 |
| 68119036020916224   | 487 |  54 | , <        | 600 |
| 68119186344771584   | 522 |  55 | . >        | 600 |
| 68119340963594240   | 558 |  56 | / ?        | 600 |
| 68119624431435776   | 624 | 229 | R Shift    | 600 |
| 68120114057707520   | 738 |  82 | Up Arrow   |   0 |

### Bottom Row (y=89)

| unique_id           | x   | hid | key        | al  |
|---------------------|-----|-----|------------|-----|
| 68117575730003968   | 147 | 224 | L Ctrl     | 600 |
| 68117794773336064   | 198 | 227 | L GUI (Win)| 600 |
| 68117988046864384   | 243 | 226 | L Alt      | 600 |
| 68118464788234240   | 354 |  44 | Space      | 600 |
| 68118954414505984   | 468 | 230 | R Alt      | 600 |
| 68119147688034304   | 513 | 231 | R GUI (Win)| 600 |
| 68119336666595328   | 557 | 240 | FN         | 600 |
| 68119521350189056   | 600 | 228 | R Ctrl     | 600 |
| 68119748983455744   | 653 |  80 | Left Arrow |   0 |
| 68119903602278400   | 689 |  81 | Down Arrow |   0 |
| 68120058221101056   | 725 |  79 | Right Arrow|   0 |

---

## Summary Statistics

| Category                    | Count |
|-----------------------------|-------|
| Total zones (Apex Pro TKL 2023) | 84 |
| OmniPoint switches (al=600) |    61 |
| Standard switches (al=0)    |    23 |
| Function row keys           |    13 |
| Nav cluster (Del/Ins/PgUp..)|     6 |
| Arrow keys                  |     4 |
| HID code 240 (FN key)       |     1 |

**Note on HID code 240 (0xF0)**: The FN key has no standard USB HID usage ID. SteelSeries assigns 0xF0 as a synthetic ID. ssgg's `KeyMappingDatabase` may need a special-case entry for this.

**Note on PrintScreen/ScrollLock/Pause**: These are absent from the Prism zone_cache for this keyboard. The Apex Pro TKL 2023 does not appear to have dedicated PrtScr/ScrLk/Pause keys; these functions are likely assigned to FN combinations.

---

## Gaps vs. ssgg Profile System

| GG Prism feature | ssgg equivalent | Status |
|-----------------|-----------------|--------|
| Three lighting states (base/idle/reactive) | Single `Effect` | **Missing** |
| Per-device configs (`config_per_device`) | Not modeled | **Missing** |
| Idle transition timer (`idleTimeMs`) | Not modeled | **Missing** |
| `globalConfig.graphics` (effect layer stack) | `Effect` enum | Partial |
| `deviceConfigs[].zoneIds` (per-key zone set) | Not modeled | **Missing** |
| `COLOR_SHIFT` effect (animated gradient) | `Effect::ColorShift` | Partial |
| `SINGLE_COLOR` effect | `Effect::Static` | Equivalent |
| `BLOOM` effect (reactive per-key bloom) | Not implemented | **Missing** |
| `spatialWavelength`, `duration`, `gradient` | Partial in `ColorShift` | Partial |
| Per-key actuation_level (OmniPoint) | Not persisted in profiles | **Missing** |
| 84-zone addressing via unique_id | No per-key zone IDs | **Missing** |

---

## Implementation Notes for ssgg

### Zone ID encoding
The `unique_id` values (e.g., `68117837727006720`) are 64-bit integers that encode the (x, y) bitmap position. The encoding scheme is not yet reverse-engineered, but decoding is not needed for implementation — ssgg can use the `hid_code` column directly (standard USB HID usage IDs from page 7) to identify keys, and generate zone IDs from `zone_cache` at runtime if needed.

### Key mapping cross-reference
The `hid_code` values in `zone_cache` are exactly the USB HID Usage IDs from page 7 (Keyboard/Keypad). These correspond directly to ssgg's `KeyMappingDatabase` entries. Cross-referencing these with `src/devices/key_mapping.rs` will allow verifying that all 84 keyboard positions are covered.

### Actuation level storage
Current ssgg has no mechanism to persist per-key actuation levels. The Prism `zone_cache` stores these alongside zone layout data. For ssgg to implement GG-compatible actuation settings, it would need a similar per-key actuation map in its profile format.

### Three-state profile model
The most impactful gap: ssgg must evolve its `Profile` → `KeyboardProfile` to support at minimum `base` and `idle` states with a configurable `idle_timeout_ms`. The `reactive` state is a separate overlay, not a replacement.

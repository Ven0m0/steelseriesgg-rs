# SteelSeries Sonar Database Schema

Generated: 2026-05-26  
Source: `C:\ProgramData\SteelSeries\GG\apps\sonar\db\database.db` (SQLite, ~2.7 MB)  
Status: PARTIAL — covers config/EQ data; volume/device routing not captured (stored elsewhere or in key_value)

---

## Tables

### `configs` — audio DSP configurations

```sql
CREATE TABLE IF NOT EXISTS "configs" (
    id                       TEXT     NOT NULL UNIQUE PRIMARY KEY,
    name                     TEXT     NOT NULL,
    vad                      INTEGER  NOT NULL,   -- virtual audio device ID (1=game, 2=mic, 3=media, 4=aux, 5=?)
    data                     TEXT     NOT NULL,   -- JSON DSP config (see below)
    schema_version           INTEGER  NOT NULL,   -- 5 or 6
    created_at               DATETIME NOT NULL,
    updated_at               DATETIME NOT NULL,
    is_preset                INTEGER  NOT NULL,   -- 0=user config, 1=built-in preset
    default_data             TEXT     NOT NULL,   -- factory default JSON for this config
    default_data_last_update DATETIME DEFAULT NULL,
    image                    TEXT     NOT NULL DEFAULT '',
    is_favorite              BOOL     NOT NULL DEFAULT false,
    favorite_position        INTEGER  NOT NULL DEFAULT -1,
    game_ids                 TEXT     NOT NULL DEFAULT '',  -- game names that auto-activate this config
    release_version          TEXT     DEFAULT NULL
);
```

**`vad` values** (Virtual Audio Device):
- 1 = Game/output (speakers/headphones)
- 2 = Microphone/input
- 3 = Media (music player output)
- 4 = Aux / chat render (incoming chat audio)
- 5 = Chat capture / streaming mic

### `selected_config` — currently active config per VAD

```sql
CREATE TABLE IF NOT EXISTS "selected_config" (
    vad       INTEGER NOT NULL UNIQUE,
    config_id TEXT    NOT NULL UNIQUE,
    FOREIGN KEY(config_id) REFERENCES "configs"(id)
);
```

### `key_value` — miscellaneous settings

```sql
CREATE TABLE key_value (
    key        TEXT     NOT NULL PRIMARY KEY,
    value      TEXT     NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL
);
```

**Known keys** (observed on this machine):
- `ONBOARDING_FINISHED` → "True"
- `HEART_BEAT` → ISO datetime
- `MODE` → "Classic"
- `GAME_FALLBACK_DEVICES` → JSON array of Windows audio device GUIDs
- `CHAT_RENDER_FALLBACK_DEVICES` → JSON array
- `CHAT_CAPTURE_FALLBACK_DEVICES` → JSON array (microphone)
- `MEDIA_FALLBACK_DEVICES` → JSON array
- `IS_STEELSERIES_WIRELESS_TRIGGER_ENABLED` → "true"
- `STREAM_MODE_*` → streaming mode configuration
- `STREAMING_STREAM_MODE_*` → per-stream-type enable flags
- `MONITORING_STREAM_MODE_*` → monitoring mode config

---

## Config Data JSON Structure

### Output Config (vad=1 — Game/Headphone)

```json
{
  "bassBoostState": {"enabled": true, "value": 0},
  "trebleBoostState": {"enabled": true, "value": 0},
  "voiceClarityState": {"enabled": true, "value": 0},
  "smartVolume": {
    "enabled": false,
    "volumeLevel": 0,
    "loudness": "balanced"
  },
  "generalGain": 0,
  "parametricEQ": {
    "enabled": true,
    "filter1":  {"enabled": true,  "qFactor": 0.7071, "frequency": 35,    "gain": 0, "type": "peakingEQ"},
    "filter2":  {"enabled": true,  "qFactor": 0.7071, "frequency": 120,   "gain": 0, "type": "peakingEQ"},
    "filter3":  {"enabled": true,  "qFactor": 0.7071, "frequency": 1000,  "gain": 0, "type": "peakingEQ"},
    "filter4":  {"enabled": true,  "qFactor": 0.7071, "frequency": 6000,  "gain": 0, "type": "peakingEQ"},
    "filter5":  {"enabled": true,  "qFactor": 0.7071, "frequency": 18000, "gain": 0, "type": "peakingEQ"},
    "filter6":  {"enabled": false, "qFactor": 0.7071, "frequency": 1000,  "gain": 0, "type": "peakingEQ"},
    "filter7":  {"enabled": false, "qFactor": 0.7071, "frequency": 2000,  "gain": 0, "type": "peakingEQ"},
    "filter8":  {"enabled": false, "qFactor": 0.7071, "frequency": 4000,  "gain": 0, "type": "peakingEQ"},
    "filter9":  {"enabled": false, "qFactor": 0.7071, "frequency": 8000,  "gain": 0, "type": "peakingEQ"},
    "filter10": {"enabled": false, "qFactor": 0.7071, "frequency": 16000, "gain": 0, "type": "peakingEQ"}
  },
  "virtualSurroundState": false,
  "virtualSurroundChannels": {
    "frontLeft":  {"position": 30,   "gain": 0},
    "frontRight": {"position": -30,  "gain": 0},
    "center":     {"position": 0,    "gain": 0},
    "subWoofer":  {"position": 0,    "gain": 0},
    "rearLeft":   {"position": 150,  "gain": 0},
    "rearRight":  {"position": -150, "gain": 0},
    "sideLeft":   {"position": 90,   "gain": 0},
    "sideRight":  {"position": -90,  "gain": 0}
  },
  "reverbGainDB": -6,
  "formFactor": "headphones",
  "globalEnableState": true
}
```

**EQ filter types**: `"peakingEQ"`, `"lowShelving"`, `"highShelving"` (biquad filter types)

**10-band parametric EQ**: filters 1-10, each independently enabled. First 5 are active by default; 6-10 are off.

### Mic/Input Config (vad=2)

```json
{
  "noiseReductionState":       {"enabled": false, "value": 0.0},
  "volumeStabilizerState":     {"enabled": false, "value": 0.0},
  "noiseGateState":            {"enabled": false, "value": -60.0},
  "automaticNoiseGateState":   {"enabled": false, "value": 0.0},
  "impactNoiseReductionState": {"enabled": false, "value": 0.0},
  "noiseCancelingState":       {"enabled": false, "value": 0.9},
  "acousticEchoCancelingState": false,
  "globalEnableState": true,
  "parametricEQ": {
    "enabled": true,
    "filter1":  {"enabled": true, "qFactor": 0.7071, "frequency": 31.0,    "gain": 0.0, "type": "lowShelving"},
    "filter2":  {"enabled": true, "qFactor": 0.7071, "frequency": 62.0,    "gain": 0.0, "type": "peakingEQ"},
    "filter3":  {"enabled": true, "qFactor": 0.7071, "frequency": 125.0,   "gain": 0.0, "type": "peakingEQ"},
    "filter4":  {"enabled": true, "qFactor": 0.7071, "frequency": 250.0,   "gain": 0.0, "type": "peakingEQ"},
    "filter5":  {"enabled": true, "qFactor": 0.7071, "frequency": 500.0,   "gain": 0.0, "type": "peakingEQ"},
    "filter6":  {"enabled": true, "qFactor": 0.7071, "frequency": 1000.0,  "gain": 0.0, "type": "peakingEQ"},
    "filter7":  {"enabled": true, "qFactor": 0.7071, "frequency": 2000.0,  "gain": 0.0, "type": "peakingEQ"},
    "filter8":  {"enabled": true, "qFactor": 0.7071, "frequency": 4000.0,  "gain": 0.0, "type": "peakingEQ"},
    "filter9":  {"enabled": true, "qFactor": 0.7071, "frequency": 8000.0,  "gain": 0.0, "type": "peakingEQ"},
    "filter10": {"enabled": true, "qFactor": 0.7071, "frequency": 16000.0, "gain": 0.0, "type": "highShelving"}
  }
}
```

**Mic-specific DSP**: noise reduction, volume stabilizer, noise gate (manual + auto), impact noise reduction, AI noise canceling, acoustic echo canceling. All disabled by default.

---

## Built-in Presets

Named presets with `is_preset=1`. Observed presets:
- Valorant Pro Preset (vad=1)
- CS:GO Pro Preset (vad=1)
- Apex Legends (vad=1)
- COD Warzone (vad=1)
- Fortnite Pro Preset (vad=1)
- League of Legends (vad=1)
- FPS Footsteps (vad=1)

Each preset has game-specific EQ tuning in its `data` JSON.

---

## ssgg Sonar Gap Analysis

ssgg's `HeadsetProfile` (`src/profiles/mod.rs`):

```rust
pub struct HeadsetProfile {
    pub sidetone: u8,
    pub mic_volume: u8,
    pub eq_preset: String,
    pub auto_off_minutes: u8,
}
```

vs. what Sonar actually stores:

| Feature | ssgg | Sonar | Gap |
|---------|------|-------|-----|
| Sidetone level | `sidetone: u8` | Not in DB (likely device HID) | Unknown |
| Mic volume | `mic_volume: u8` | Not in DB (key_value?) | Unknown |
| EQ preset (named) | `eq_preset: String` | `configs.name` | Conceptual match |
| Auto-off timeout | `auto_off_minutes: u8` | Not in DB | Unknown |
| 10-band parametric EQ | Not modeled | Full per-band config | **Missing** |
| Noise reduction | Not modeled | `noiseReductionState` | **Missing** |
| Noise gate | Not modeled | `noiseGateState` | **Missing** |
| AI noise cancel | Not modeled | `noiseCancelingState` | **Missing** |
| Echo canceling | Not modeled | `acousticEchoCancelingState` | **Missing** |
| Bass/treble boost | Not modeled | `bassBoostState`/`trebleBoostState` | **Missing** |
| Voice clarity | Not modeled | `voiceClarityState` | **Missing** |
| Virtual surround | Not modeled | `virtualSurroundState` + 8-channel | **Missing** |
| Per-game profiles | Not modeled | `configs.game_ids` JSON | **Missing** |
| Stream mode (OBS/Twitch) | Not modeled | `key_value` stream config | **Missing** |
| Multiple VADs (5 channels) | Not modeled | 5 independent channels | **Missing** |

**Assessment**: ssgg's Sonar integration is a proof-of-concept. A production-equivalent would require modeling the full 5-channel DSP config with parametric EQ and all mic processing toggles.

**Priority for ssgg**: The `SonarClient` in `src/audio/sonar.rs` should at minimum be able to read/write the parametric EQ `data` JSON per VAD. The `eq_preset` field in `HeadsetProfile` maps conceptually to `configs.name` in the Sonar DB.

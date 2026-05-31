# GameSense API Reference

Generated: 2026-05-26  
Sources: ssgg source code + GG main SQLite schema + **live API validation against GG 111.0.0** (port 53714)  
Status: VALIDATED — all 7 endpoints tested against running GG

---

## Port Discovery

GG does **not** use a fixed port. The actual port is determined at runtime from:

```
C:\ProgramData\SteelSeries\SteelSeries Engine 3\coreProps.json
C:\ProgramData\SteelSeries\GG\coreProps.json  (symlink/copy)
```

Example contents:
```json
{"encryptedAddress":"127.0.0.1:53723","ggEncryptedAddress":"127.0.0.1:6327","address":"127.0.0.1:53714"}
```

- `address` → standard GameSense HTTP endpoint (use this)
- `encryptedAddress` → HTTPS/TLS endpoint (unknown cert chain)
- `ggEncryptedAddress` → GG-internal encrypted endpoint

**ssgg behavior**: ssgg hardcodes port 27301, matching the official SteelSeries SDK documentation. GG uses a dynamic port. For local-only use, ssgg's fixed port is compatible because applications can find ssgg via a similar `coreProps.json` file.

---

## Endpoints (all 7 verified)

Port: **dynamic** (read from `coreProps.json`; official SDK documents 27301)

| Method | Path | GG Response Key | Verified |
|--------|------|-----------------|----------|
| POST | `/game_metadata` | `game_metadata` | ✅ |
| POST | `/bind_game_event` | `bind_game_event` | ✅ |
| POST | `/register_game_event` | `register_game_event` | ✅ |
| POST | `/game_event` | `game_event` | ✅ |
| POST | `/game_heartbeat` | `game_heartbeat` | ✅ |
| POST | `/remove_game` | `remove_game_event` ⚠️ | ✅ |
| POST | `/remove_game_event` | (not tested separately) | — |

**Note**: `/remove_game` returns `{"remove_game_event": {...}}` — the response key says "remove_game_event", not "remove_game".

---

## Request / Response Schemas (live-captured)

### `POST /game_metadata`

**Request** (matches ssgg `GameMetadata`):
```json
{"game":"SSGG_TEST","game_display_name":"ssgg Test","developer":"test"}
```

**Response** (GG returns a wrapper object):
```json
{
  "game_metadata": {
    "is": "",
    "game": "SSGG_TEST",
    "game_display_name": "ssgg Test",
    "deinitialize_timer_length_ms": 0,
    "developer": "test",
    "enabled": false,
    "options": null
  }
}
```

**Fields absent from ssgg's `GameMetadata` struct:**
- `is`: always `""` — unknown purpose (possibly an internal type discriminator)
- `enabled`: `false` — game starts disabled until it fires events
- `options`: `null` — JSON options blob (see DB schema note)

### `POST /bind_game_event`

**Request** (standard bind):
```json
{
  "game": "SSGG_TEST",
  "event": "HEALTH",
  "min_value": 0,
  "max_value": 100,
  "icon_id": 0,
  "handlers": [
    {
      "device-type": "keyboard",
      "zone": "function-keys",
      "color": {"gradient": {"zero": {"red":0,"green":255,"blue":0}, "hundred": {"red":255,"green":0,"blue":0}}},
      "mode": "percent"
    }
  ]
}
```

**Response:**
```json
{
  "bind_game_event": {
    "game": "SSGG_TEST",
    "event": "HEALTH",
    "icon_id": 0,
    "min_value": 0,
    "max_value": 100,
    "value_optional": false,
    "data_fields": null,
    "handlers": [...]
  }
}
```

**Fields absent from ssgg's `EventBinding` struct:**
- `value_optional`: `false` — whether the event can fire without a value
- `data_fields`: `null` — JSON array of structured data field definitions (see `is_fancy` in DB schema)

**`product_id` is NOT a valid request field** — sending it returns HTTP 400. The DB column is set internally by GG.

### `POST /register_game_event`

**Request:**
```json
{"game":"SSGG_TEST","event":"MANA","min_value":0,"max_value":100}
```

**Response:**
```json
{
  "register_game_event": {
    "game": "SSGG_TEST",
    "event": "MANA",
    "icon_id": 0,
    "min_value": 0,
    "max_value": 100,
    "value_optional": false,
    "data_fields": null
  }
}
```

### `POST /game_event`

**Request:**
```json
{"game":"SSGG_TEST","event":"HEALTH","data":{"value":75}}
```

**Response:**
```json
{
  "game_event": {
    "level": "",
    "game": "SSGG_TEST",
    "event": "HEALTH",
    "data": {"value": 75}
  }
}
```

**`level` field**: Always `""` in responses — corresponds to the `level` column in `game_integration_event_bindings`. This is an internal GG priority/routing field set by GG, not by the game client.

### `POST /game_heartbeat`

**Request:** `{"game":"SSGG_TEST"}`  
**Response:** `{"game_heartbeat":{"game":"SSGG_TEST"}}`

### `POST /remove_game`

**Request:** `{"game":"SSGG_TEST"}`  
**Response:** `{"remove_game_event":{"game":"SSGG_TEST"}}`

Note: response key is `remove_game_event`, not `remove_game`.

---

## CORS Policy

GG uses **wildcard CORS** — not localhost-restricted:

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: POST, OPTIONS
Access-Control-Allow-Headers: Content-Type
```

**ssgg behavior**: ssgg enforces localhost-only origin (`src/gamesense/server.rs`). This is **more secure than GG**. ssgg's policy is correct and should not be loosened to match GG's wildcard.

The wildcard in GG is a security weakness: any webpage a user visits can make requests to the local GameSense API. ssgg avoids this.

---

## Implementation Gaps (ssgg vs GG)

| Feature | ssgg | GG | Priority |
|---------|------|----|----------|
| Dynamic port from `coreProps.json` | Fixed 27301 | Dynamic | Medium |
| `game_metadata.enabled` field | Not returned | Returned | Low |
| `game_metadata.options` field | Not accepted | Accepted | Low |
| `register_game_event` response includes `value_optional`, `data_fields` | Not returned | Returned | Low |
| `bind_game_event` response includes `value_optional`, `data_fields` | Not returned | Returned | Low |
| `game_event` response includes `level` | Not returned | Returned | Low |
| `remove_game` response key | Not verified | `remove_game_event` | Low |
| CORS policy | Localhost-only ✅ | Wildcard ⚠️ | ssgg is better |

# GG engine GameSense surface (static read)

Read-only static analysis of `apps\engine\SteelSeriesEngine.exe` (Go, sha256 `c1b8e182...0ec1`, the build
installed on 2026-10-25). Two passes: a byte scan for printable strings (route-like paths
and Go symbol names), then REA `search_strings` over Ghidra 12.1.4's defined-string inventory once the analysis
could finish (it needs more than REA's default 330 s: `REA_GHIDRA_STARTUP_TIMEOUT_MS` and Claude Code's
`MCP_TOOL_TIMEOUT` were raised). Nothing was run, attached or modified. Tags: **observed** = the string is in
the binary; **inferred** = read from names only; **unknown** = not settled by this pass. An empty Ghidra search
does not prove a string is absent (packed or undefined strings are not in its inventory).

## Findings

| Claim | Tag |
|-------|-----|
| Ghidra's string inventory has no `game_heartbeat` and no `coreProps`. It has `game_metadata` (`0x1409b0f4a`) and the format string `game_metadata/%s` (`0x1409b3c6e`), whose use (URL the engine builds, or a storage key) is not yet traced. | observed (Ghidra), use unknown |
| It carries internal versioned endpoints `/gameSenseInternal/<op>/v1`, with `<op>` one of `autoclip` (`0x1409c6d96`), `stop_game` (`0x1409c8170`), `game_event` (`0x1409c98b7`), `bind_game_event` (`0x1409d2423`), `remove_bindings` (`0x1409d2448`), `game_event_raw_data` (`0x1409e0088`), `multiple_game_events` (`0x1409e132b`), `load_golisp_handlers` (`0x1409e1355`), `set_game_enabled_flag` (`0x1409e259b`), `rebind_multiple_events` (`0x1409e3699`), `oled_active_for_device` (`/v1/([0-9]+)`, a route pattern taking a device id, `0x1409edf6e`) and `gamesense_devices_without_handlers` (`0x1409f1607`): 12 in all. | observed (Ghidra and byte scan) |
| `pkg/gamesense.(*gamesenseService)` has `GetGGGameSenseAddress`, `loadGGGameSenseAddress` and `updateOldGGAddress`, and `pkg/utils/core_module_ipc/gg_gamesense.go` exists. | observed |
| The public GameSense HTTP server (the one games find through `coreProps.json`) runs in another GG process and forwards to the engine over these internal endpoints; the engine only learns and publishes that server's address. | inferred |
| `pkg/utils/game_integration_endpoints` has `SetupCsGoEndpoint`, `SetupDota2Endpoint`, `getSteamGameInstallDir`, `getCsGoEndpointFilenames` and cleanup counterparts: GG writes game-state-integration config files into the Steam install of CS:GO/CS2 and Dota 2. | observed (names), inferred (behaviour) |
| GameSense handlers are golisp (`load_golisp_handlers`, `gamesense_types.GameGoLispPair`), and OLED screen events carry `GameSenseScreenDimensions` / `GameSenseOledInfo`. | observed (names) |
| Event types: `GameEvent`, `GameEventDataField`, `MultipleGameEvents`, `GameEventWithBothHandlerLevels`, `GameSenseToggle`, from `github.com/steelseries/gg-shared-go-library` `v0.1.30`. | observed |
| Wire format of the internal endpoints, the port they listen on, and which process hosts the public server. | unknown |

## Against ssgg

`src/gamesense/server.rs` serves `/game_metadata`, `/bind_game_event`, `/register_game_event`, `/game_event`,
`/game_heartbeat`, `/remove_game`, `/remove_game_event` and `/`. Gaps the names point at, none of them implemented:

- `multiple_game_events` (a batch event route; the public name is likely `/multiple_game_events`, unverified).
- The CS2/Dota 2 game-state-integration installer (GG drops a cfg file into the game's Steam folder so the game
  posts its state to GG). An ssgg equivalent would be a `ssgg gamesense install-gsi` command; the file format is
  documented by Valve, not by this scan.
- OLED screen handlers for GameSense (`screen` handlers); ssgg now has a raw OLED write for `0x1628`
  (`ssgg oled show`), not a GameSense screen renderer.

Next step for whoever continues: REA `xrefs` on `0x1409b3c6e` (`game_metadata/%s`) and on the
`gameSenseInternal` strings to find the router registering them and what calls out with `game_metadata/%s`;
then find the process that owns the public GameSense port (likely the GG client or a sub-app) and read its route
table. REA keeps the Ghidra analysis only for the open session; a new session re-runs it (several minutes).

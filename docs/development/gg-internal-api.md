# SteelSeries GG Internal Management API

Generated: 2026-05-26  
Source: `GG.API.dll` decompiled via PowerShell reflection (Assembly.LoadFrom)  
Port: `ggEncryptedAddress` / `encryptedAddress` from `coreProps.json` (TLS-only, not plaintext)

**This is NOT the GameSense API.** The GameSense game integration endpoints (`/game_metadata`, etc.) are served by the SteelSeriesEngine sub-app (Python/cvgamesense), not this API.

---

## Complete Route Table

Extracted from GG.API, Version=1.1.26.0

| Method | Route | Controller | Handler | Notes |
|--------|-------|-----------|---------|-------|
| GET | `/health` | HealthCheckController | `GetHealth()` | Liveness check |
| GET | `/settings` | ApplicationSettingsController | `Get()` | Get app settings |
| POST | `/settings` | ApplicationSettingsController | `Post(PostBody)` | Set app settings |
| POST | `/settings/datetime` | ApplicationSettingsController | `PostDateTime(PostDateTimeBody)` | Set system datetime |
| GET | `/{locale}` | LocalesController | `GetLocale()` | Get current locale |
| GET | `/locales` | LocalesController | `GetSupportedLocales()` | List supported locales |
| POST | `/shutdown` | ShutdownController | `Shutdown()` | Graceful GG shutdown |
| POST | `/admin/macroEvents` | AdminController | `MacroEvents(MacroEventsBody)` | Inject macro events |
| POST | `/analytics/trackWithoutGG` | AnalyticsController | `TrackWithoutGG(TrackWithoutGGRequest)` | Anonymous analytics |
| POST | `/inputlib/eventState` | InputLibController | `EventState(EventStateOptionsBody)` | InputLib event config |
| POST | `/keyboardShortcut` | KeyboardShortcutController | `SetKeyboardShortcut(KeyboardShortcut)` | Register keyboard shortcut |
| GET | `/subApps` | SubAppsController | `GetSubApps()` | List sub-apps (Prism, Sonar...) |
| POST | `/subApps` | SubAppsController | `CreateSubApp(SubAppConfiguration)` | Register a new sub-app |
| POST | `/subApps/status` | SubAppsController | `PostSubAppStatus(SubAppStatusBody)` | Sub-app heartbeat/status |
| POST | `/subApps/ready` | SubAppsController | `PostSubAppReady(SubAppReadyBody)` | Signal sub-app ready |
| POST | `/subApps/register` | SubAppsController | `PostSubAppRegister(SubAppRegisterBody)` | Full sub-app registration |
| POST | `/subApps/{subApp}/unlockFeature` | SubAppsController | `PostSubAppUnlockFeature(String, ...)` | Enable sub-app feature |
| POST | `/taskbar/menu` | TaskbarController | `SetMenu(SetMenuPayload)` | Set system tray menu |
| POST | `/taskbar/clearMenu` | TaskbarController | `ClearMenu(ClearMenuPayload)` | Clear system tray menu |
| POST | `/taskbar/notification` | TaskbarController | `Notification(DisplayNotificationPayload)` | Show tray notification |
| POST | `/taskbar/icon` | TaskbarController | `Icon(SetIconPayload)` | Set tray icon |
| POST | `/universalTriggers/clear` | UniversalTriggersController | `Clear()` | Clear all triggers |
| POST | `/universalTriggers/add` | UniversalTriggersController | `Add(IList<UniversalTrigger>)` | Add macro triggers |
| POST | `/universalTriggers/remove` | UniversalTriggersController | `Remove(IList<UniversalTrigger>)` | Remove triggers |
| WS | `/ws` | WebSocketController | `ConnectSock()` | Management WebSocket |
| WS | `/eventing` | WebSocketController | `ConnectEventing()` | Event stream WebSocket |
| POST | `/admin/macroRecording/start` | MacroRecordingContoller | `StartMacroRecording(...)` | Begin macro recording |
| POST | `/admin/macroRecording/stop` | MacroRecordingContoller | `StopMacroRecording()` | Stop macro recording |

---

## Key Types (from GG.API.dll)

### SubAppRegisterBody
Used by sub-apps (Prism, Sonar, Moments) to register themselves with GG on startup. Contains their web server address so GG can proxy requests.

### SubAppConfiguration
```
Name: string
IsEnabled: bool
IsWindowsSupported: bool
ExecutableName: string
ToggleViaSettings: bool
AutoStart: bool
```

### EventStateOptionsBody
Controls which InputLib callback types are active (`ILCT_HIDINPUT`, `ILCT_UNIVTRIGGER`, etc.).

---

## Architecture Implications for ssgg

GG's architecture is a microservices model:
1. **GG main process** (`SteelSeriesGGEZ.exe`) — orchestrates sub-apps via this management API
2. **Sub-apps** — Prism (RGB), Sonar (audio), Moments (video), Engine (legacy protocol) — each runs as a separate process
3. **GameSense** — served by the Engine sub-app (cvgamesense Python service), NOT by GG.API.dll

ssgg is a monolith that implements the GameSense API, RGB control, and audio in a single process — architecturally simpler and appropriate for an open-source alternative.

The WebSocket endpoint (`/eventing`) streams device events (hotplug, button presses, state changes) in real-time. This is how GG's UI reacts to device events without polling. ssgg's equivalent is its internal channel-based event system.

---

## InputLib HID Write Path (confirmed via reflection)

From `GG.Interop.Services.dll`:
```csharp
public interface IInputLibInteropService {
    uint Initialize();
    uint SendCommand(uint protocol, byte[] byteArray, uint size);
    uint SetCallback(InputLibCallbackType callbackType, InputLibCallback callback);
}
```

From `InputLib.dll` (native, string extracted):
```
InputLib_SendCommand   (exported function)
?SendCommand@InputLib@@QEAAIIPEAEI@Z   (mangled: InputLib::SendCommand(uint, uchar*, uint) -> uint)
```

The managed `IInputLibInteropService.SendCommand(protocol, bytes, size)` calls the native `InputLib_SendCommand` via P/Invoke. The `protocol` parameter likely identifies the USB interface/device type.

**For x64dbg**: Set a breakpoint on `InputLib_SendCommand` in `InputLib.dll`. When triggered by an RGB change in GG UI, inspect the `byte[]` argument (second parameter, pointed to by RDX register on x64 calling convention) to capture the raw HID report.

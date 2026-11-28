---
description: Scaffold a new SteelSeries device — creates the implementation file and wires it into the registry
---

Scaffold a new device for steelseriesgg-rs. Ask the user for:
1. Device name (e.g. `Apex 5`)
2. Device type: `keyboard`, `headset` or `mouse`
3. USB vendor ID and product ID (hex, e.g. `0x1038`, `0x1234`)

Then create the following, using existing devices as templates:

Follow `.claude/rules/device-capabilities.md`: a new model is a table row, not a product ID check in code, and
every yes/no capability is a `can_*` field with a plain doc comment.

In every case, add the product ID to `product_ids` in `src/devices/mod.rs`.

**For a keyboard:**
- Add a row to `KEYBOARD_MODELS` (`src/devices/keyboards/models.rs`): name, class, direct frame and the
  behaviour properties it needs (interfaces, zone count, `can_*` flags). A wired Gen 3 board also gets a
  `MODEL_CAPS` row (`src/devices/keyboards/model_caps.rs`).
- For per-key colours, add a mapping to `KeyMappingDatabase` (`src/devices/key_mapping.rs`); for a measured zone
  layout, add it to `src/devices/zone_mapping.rs`.
- Only if no existing class speaks its protocol: `src/devices/keyboards/<snake_case_name>.rs` implementing the
  `Keyboard` trait (and its `Device` supertrait), with a new `KeyboardClass` variant.
  - Stub all methods, mark protocol work `[EXPERIMENTAL]` or `[PLACEHOLDER]`
  - Use `HidReportBuilder` for all HID output — never raw bytes

**For a headset:**
- If it speaks the Nova 7 frames: a row in `NOVA7_MODELS` (`src/devices/headsets/nova7.rs`).
- Otherwise `src/devices/headsets/<snake_case_name>.rs` implementing the `Headset` trait, with its own model table
  and a lookup of that table in `DeviceManager::open_headset` (`src/devices/discovery.rs`); without it the headset
  falls back to `GenericHeadset`.
- Add the name and type arms in `src/devices/mod.rs`.

**For a mouse:**
- A row in `MOUSE_MODELS` (`src/devices/mice/modern.rs`) with its `MouseSpec`, plus the name and type arms in
  `src/devices/mod.rs`. A mouse outside the Modern family needs its own driver, its own table and a lookup in
  `DeviceManager::open_mouse`.

Then run `node tools/profile-gui/sync-products.mjs` to refresh the editor's product table.

**After creating files:**
1. Run `cargo clippy --all-targets --locked -- -D warnings` and fix any errors
2. Run `cargo fmt --all`
3. Report the files created and the VID/PID that was registered

Label all protocol implementation as `[EXPERIMENTAL]` until tested on hardware.

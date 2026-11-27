# Device Capability Rules

Code that handles a difference between devices describes the behaviour, not the device. A new model must be a new
row in a model table, not a new code branch.

## In code

1. Put each device difference in the model table as a behaviour property: an interface number, a zone count, a
   DPI preset, a frame variant, a "write confirmed on hardware" flag.
2. Let callers ask the model table for the behaviour, as discovery asks a `KeyboardModel` for its
   `only_control_interface` and the keyboard code for its `zone_count`.
3. Do not add product ID checks outside the model tables: no `if pid == X`, no `matches!(pid, A | B)`, no
   `is_<model>()` helpers.
4. Keep product ID constants in `devices::product_ids` (`src/devices/mod.rs`) and in the model tables. Tests may
   use them.
5. A protocol family that needs its own driver is picked from its table (a family enum or a table lookup), not by a
   product ID check in the caller.

## Names and descriptions

A reader must understand a capability at the call site, without the protocol notes and without guessing.

1. A yes/no capability starts with `can_` and says what `ssgg` or the device can do, in positive form:
   `can_write_key_mappings`, `can_set_actuation_live`, `can_read_profile`. Do not use `is_<model>`, `supports_x`,
   `live_analog` or other names that only someone who knows the protocol can read.
2. A capability that holds a value is named for that value: `zone_count`, `first_preset`,
   `only_control_interface`.
3. Each capability has a doc comment with:
   - one plain sentence on what it means for the user or the caller, for example "ssgg may write a profile to the
     keyboard's memory";
   - where the value comes from: which device, test and date confirmed it, or that it is `[EXPERIMENTAL]`.

   A capability is a field or method that decides what `ssgg` sends to a device or allows the user to do.
   Identity fields (`product_id`, `name`) are not capabilities. Neither are facts about the frame layout or the
   connection (`DpiSpec::per_axis`, `DirectFrame::init`, `MouseModel::dongle`, `DirectLighting::probes_firmware`):
   they keep a name that says what they are, and their doc gives the source. When every field of a table has the
   same source, the struct's doc comment may give it once; a field then adds only what differs, such as a confirmed
   value.
4. A function or method that answers a capability follows the same names and has the same kind of doc comment.

Example:

```rust
/// `ssgg keymap set` can change what a key does. Confirmed on the Apex Pro TKL 2023 `0x1628` for the base layer
/// (Windows, 2026-11-11); the other layers are `[EXPERIMENTAL]`. `false` on every board where nobody has tested
/// the write.
pub can_write_key_mappings: bool,
```

```rust
if caps.can_write_key_mappings {
    // ...
}
```

## In the profile editor (`ssgg-gui`)

1. The page gets behaviour properties from the server (`capabilities`), never per-model text.
2. Text that the device panels show (the Headset tab, the Device control panel and any later device tab) names no
   model: hints, notes, labels, and messages from `src/gui.rs`. The profile editor may name the file format it edits and the board it was tested
   on, because it edits that board's profile file.
3. Test-status notices for one model stay in the CLI (`ssgg mouse`, `ssgg headset`). The Device control panel
   shows the CLI's output unchanged, so that output may name a model; the panel's own labels and notes may not.
4. The page does not check product IDs. It may show the product ID and the model name as identification.

## Existing code

The product ID checks that chose a behaviour were moved to `can_` flags and table properties when this rule was
added (2026-11-28); tests in `keyboards/model_caps.rs`, `keyboards/models.rs`, `headsets/nova7.rs`,
`headsets/mod.rs` and `mice/modern.rs` pin which models set each flag. These uses of a
product ID are not behaviour checks and stay:

- the model tables themselves, `product_ids`, the name and type maps, and identity comparisons (the same device,
  a `--product-id` filter);
- per-model file names for saved state (`direct-brightness-<pid>.bin`, `actuation-<pid>-...`);
- the key-name lookups of `ssgg keymap` and `ssgg actuation`, which read the `0x1628` key table before a
  keyboard is opened;
- `devices::is_detect_only`, the list of devices that `ssgg` never writes to;
- `headsets::nova7::HARDWARE_CONFIRMED`, the list that picks the CLI's test-status notice for a headset;
- `LEGACY_ONBOARD_PIDS` in `tools/profile-gui/app.js`, the profile editor's own copy of the on-board family, used
  only when the generated product table is missing.

When a change touches one of these and the rule can apply without a large change, apply it.

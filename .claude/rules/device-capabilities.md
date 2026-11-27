# Device Capability Rules

Code that handles a difference between devices describes the behaviour, not the device. A new model must be a new
row in a model table, not a new code branch.

## In code

1. Put each device difference in the model table as a behaviour property: an interface number, a zone count, a
   DPI preset, a frame variant, a "write confirmed on hardware" flag.
2. Let callers ask the model table for the behaviour, as the mouse code asks a `MouseModel` for its interface
   number and zone count.
3. Do not add product ID checks outside the model tables: no `if pid == X`, no `matches!(pid, A | B)`, no
   `is_<model>()` helpers.
4. Keep product ID constants in `src/devices/product_ids` and in the model tables. Tests may use them.
5. A protocol family that needs its own driver is picked from its table (a family enum or a table lookup), not by a
   product ID check in the caller.

## Names and descriptions

A reader must understand a capability at the call site, without the protocol notes and without guessing.

1. A yes/no capability starts with `can_` and says what `ssgg` or the device can do, in positive form:
   `can_write_profile`, `can_set_actuation_live`, `can_read_firmware`. Do not use `is_<model>`, `supports_x`,
   `live_analog` or other names that only someone who knows the protocol can read.
2. A capability that holds a value is named for that value: `zone_count`, `dpi_preset`, `control_interface`.
3. Each capability has a doc comment with:
   - one plain sentence on what it means for the user or the caller, for example "ssgg may write a profile to the
     keyboard's memory";
   - where the value comes from: which device, test and date confirmed it, or that it is `[EXPERIMENTAL]`.
4. A function or method that answers a capability follows the same names and has the same kind of doc comment.

Example:

```rust
/// `ssgg` may write an on-board profile to the keyboard's memory. Confirmed on the Apex Pro TKL 2023 `0x1628`
/// (Windows, 2026-11-10); `false` on every board where nobody has tested the write.
pub can_write_profile: bool,
```

```rust
if caps.can_write_profile {
    // ...
}
```

## In the profile editor (`ssgg-gui`)

1. The page gets behaviour properties from the server (`capabilities`), never per-model text.
2. Text that the page itself shows names no model: hints, notes, labels, and messages from `src/gui.rs`.
3. Test-status notices for one model stay in the CLI (`ssgg mouse`, `ssgg headset`).
4. The page does not check product IDs. It may show the product ID and the model name as identification.

## Existing code

The product ID checks that chose a behaviour were moved to `can_` flags and table properties when this rule was
added (2026-11-28); tests in `model_caps.rs` and `models.rs` pin which models set each flag. These uses of a
product ID are not behaviour checks and stay:

- the model tables themselves, `product_ids`, the name and type maps, and identity comparisons (the same device,
  a `--product-id` filter);
- per-model file names for saved state (`direct-brightness-<pid>.bin`, `actuation-<pid>-...`);
- the key-name lookups of `ssgg keymap`, which read the `0x1628` key table before a keyboard is opened.

When a change touches one of these and the rule can apply without a large change, apply it.

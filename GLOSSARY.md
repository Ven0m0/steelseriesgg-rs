# Glossary

Domain terms used in code, docs and reviews. Source code wins over this file.

**Keyboard model**: the per-product-ID row in `KEYBOARD_MODELS` (`src/devices/keyboards/models.rs`). It
decides the class discovery builds, the registry name, the raw-hidraw fallback and the direct frame. A
product ID without a row is not a keyboard to discovery.

**Keyboard class**: what a keyboard model is built as (`KeyboardClass`): `ApexProTkl2023` for the
direct-lighting models, `Apex7` for the on-board profile protocol, `Apex3`, `Apex3Tkl`, `ApexM750`, or
`Generic`. The class decides what a keyboard can do beyond lighting.

**Direct frame**: a model's live lighting report (`DirectFrame`): packet id (`0x3A`, `0x40` or `0x61`),
report length, key table, and whether the `0x4B` init report precedes it. All `[EXPERIMENTAL]`; see
`.claude/rules/experimental-protocol.md`. A model either has one fixed frame or two picked by a firmware
probe (`DirectLighting::ByFirmware`, 1.19.7 and later = Gen 3).

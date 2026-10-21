//! Optional key remapper: rewrite a SteelSeries keyboard's key events and re-emit them.
//!
//! The key table, bindings and state machine are plain Rust and compile everywhere (config
//! validation and `ssgg remap list` need them). Only the evdev/uinput layer is behind the `remap`
//! feature and Linux. [EXPERIMENTAL] Never run against real hardware as of this change.

pub mod binding;
pub mod keys;
pub mod remapper;

#[cfg(all(feature = "remap", target_os = "linux"))]
pub mod linux;

pub use binding::{Action, BindingSet, MAX_COMBO_LEN};
pub use keys::InputKey;
pub use remapper::{ESCAPE_CHORD, Effect, KeyEvent, KeyValue, Remapper, RemapperConfig};

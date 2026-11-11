//! Command handlers split out of `main.rs`: the families that change rarely.

mod apex_2023;
mod diagnostics;
mod gen3_profile;
mod keymap;
mod onboard;
mod performance;

pub(crate) use apex_2023::{cmd_actuation, cmd_oled};
pub(crate) use diagnostics::{cmd_bug_report, cmd_hid_logs, cmd_status, cmd_validate};
pub(crate) use keymap::{cmd_device, cmd_keymap};
pub(crate) use onboard::cmd_onboard;
pub(crate) use performance::{cmd_performance, cmd_test_device, cmd_verify_performance};

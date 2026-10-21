//! Command handlers split out of `main.rs`: the families that change rarely.

mod diagnostics;
mod onboard;
mod performance;

pub(crate) use diagnostics::{cmd_bug_report, cmd_hid_logs, cmd_status, cmd_validate};
pub(crate) use onboard::cmd_onboard;
pub(crate) use performance::{cmd_performance, cmd_test_device, cmd_verify_performance};

//! `[EXPERIMENTAL]` Apex Pro TKL 2023 (`0x1628`) commands read from GG's device spec: live analog-key
//! settings and the OLED. Nothing here is confirmed on hardware.

use steelseries_gg::devices::hid_reports::{
    Apex2022DisplayCommand, HallLayer, HidReportBuilder, PROFILE_CHUNK_REPORT_SIZE,
};
use steelseries_gg::devices::keyboards::apex_pro_tkl_2023::{AnalogSetting, analog_report, key_actuation_setting};
use steelseries_gg::devices::{DeviceManager, DeviceType, HidDeviceType, KeyMappingDatabase, product_ids};
use steelseries_gg::{Error, Result};

use crate::cli::{ActuationAction, OledAction, parse_key_name};

/// Print a report as hex, report-ID byte first, with its zero tail summarised.
pub(crate) fn print_report(report: &[u8]) {
    let used = report.iter().rposition(|&b| b != 0).map_or(1, |i| i + 1);
    for line in report[..used].chunks(16) {
        println!(
            "  {}",
            line.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
        );
    }
    if used < report.len() {
        println!(
            "  ... then {} zero bytes ({} in all)",
            report.len() - used,
            report.len()
        );
    }
}

/// `KEY=MM` -> (HID usage on the Apex Pro TKL 2023, tenths of a mm).
fn key_change(pair: &str) -> Result<(u8, u8)> {
    let (name, mm) = pair
        .split_once('=')
        .ok_or_else(|| Error::InvalidConfig(format!("{pair}: expected KEY=MM, e.g. W=1.0")))?;
    let key = parse_key_name(name).ok_or_else(|| Error::InvalidConfig(format!("unknown key name {name}")))?;
    let hid = KeyMappingDatabase::new()
        .get_mapping(product_ids::APEX_PRO_TKL_2023)
        .and_then(|mapping| mapping.get_key_address(key))
        .map(|address| address.hid_code)
        .ok_or_else(|| Error::InvalidConfig(format!("{name} is not a key of the Apex Pro TKL 2023")))?;
    let mm: f32 = mm.parse().map_err(|e| Error::InvalidConfig(format!("{pair}: {e}")))?;
    if !(0.1..=4.0).contains(&mm) {
        return Err(Error::InvalidConfig(format!("{pair}: actuation must be 0.1-4.0 mm")));
    }
    Ok((hid, (mm * 10.0).round() as u8))
}

fn setting_for(action: &ActuationAction) -> Result<(AnalogSetting, bool)> {
    let layer = |second: bool| if second { HallLayer::Second } else { HallLayer::Primary };
    Ok(match action {
        ActuationAction::Set { mm, second, dry_run } => {
            if !(0.1..=4.0).contains(mm) {
                return Err(Error::InvalidConfig(
                    "Actuation point must be between 0.1mm and 4.0mm".to_string(),
                ));
            }
            let tenths_mm = Some((mm * 10.0).round() as u8);
            (
                AnalogSetting::Actuation {
                    layer: layer(*second),
                    tenths_mm,
                },
                *dry_run,
            )
        }
        ActuationAction::SetValue { value, second, dry_run } => {
            if !(1..=40).contains(value) {
                return Err(Error::InvalidConfig(
                    "Actuation point value must be between 1 and 40".to_string(),
                ));
            }
            let tenths_mm = Some(*value);
            (
                AnalogSetting::Actuation {
                    layer: layer(*second),
                    tenths_mm,
                },
                *dry_run,
            )
        }
        ActuationAction::SetKey { keys, second, dry_run } => {
            let changes = keys.iter().map(|pair| key_change(pair)).collect::<Result<Vec<_>>>()?;
            (key_actuation_setting(layer(*second), &changes)?, *dry_run)
        }
        ActuationAction::ClearSecond { dry_run } => (
            AnalogSetting::Actuation {
                layer: HallLayer::Second,
                tenths_mm: None,
            },
            *dry_run,
        ),
        ActuationAction::ReleaseMode { mode, dry_run } => {
            let mode = mode
                .parse()
                .map_err(|e| Error::InvalidConfig(format!("release mode: {e}")))?;
            (AnalogSetting::ReleaseMode(mode), *dry_run)
        }
        ActuationAction::RapidTrigger { sensitivity, dry_run } => {
            (AnalogSetting::RapidTriggerSensitivity(*sensitivity), *dry_run)
        }
        ActuationAction::RapidTap { state, dry_run } => (AnalogSetting::RapidTap(state == "on"), *dry_run),
        ActuationAction::RapidTapPairs { pairs, dry_run } => (AnalogSetting::RapidTapPairs(pairs.clone()), *dry_run),
    })
}

pub(crate) async fn cmd_actuation(manager: &DeviceManager, action: ActuationAction) -> Result<()> {
    let (setting, dry_run) = setting_for(&action)?;
    if dry_run {
        let (report, feature) = analog_report(&setting)?;
        let kind = if feature { "feature" } else { "output" };
        println!("dry run, nothing sent: Apex Pro TKL 2023 (0x1628) {kind} report [EXPERIMENTAL]");
        print_report(&report);
        return Ok(());
    }

    let keyboard_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| Error::Other("No keyboard found".to_string()))?;
    println!("Using keyboard: {}", keyboard_info.name);
    let mut keyboard = manager.open_keyboard(keyboard_info)?;

    if keyboard_info.product_id == product_ids::APEX_PRO_TKL_2023 {
        crate::warn_if_gg_engine_running();
        // No APPLY/initialize: GG sends neither around these frames, and direct mode ignores APPLY.
        let reply = keyboard.send_analog_setting(&setting)?;
        println!(
            "Sent [EXPERIMENTAL]: from GG's device spec, never confirmed on hardware; whether it survives \
             unplugging is unknown"
        );
        match reply {
            Some(reply) => {
                println!("Board answered:");
                print_report(&reply);
            }
            None => println!("No 0x38 answer from the board"),
        }
        return Ok(());
    }

    let AnalogSetting::Actuation {
        layer: HallLayer::Primary,
        tenths_mm: Some(value),
    } = setting
    else {
        return Err(Error::InvalidConfig(
            "this setting is only defined for the wired Apex Pro TKL 2023 (0x1628)".to_string(),
        ));
    };
    keyboard.initialize()?;
    keyboard.set_actuation_point(value)?;
    keyboard.apply().await?;
    println!("Sent the unverified 0x2D actuation command (a confirmed no-op on the Apex Pro TKL 2023)");
    Ok(())
}

pub(crate) async fn cmd_oled(manager: &DeviceManager, action: OledAction) -> Result<()> {
    let bitmap: Option<[u8; 640]> = match &action {
        OledAction::Show { file, .. } => {
            let bytes = tokio::fs::read(file)
                .await
                .map_err(|e| Error::Other(format!("read {}: {e}", file.display())))?;
            Some(bytes.as_slice().try_into().map_err(|_| {
                Error::InvalidConfig(format!(
                    "{} is {} bytes, an OLED frame is 640",
                    file.display(),
                    bytes.len()
                ))
            })?)
        }
        OledAction::Reset { .. } => None,
    };
    let command = bitmap
        .as_ref()
        .map_or(Apex2022DisplayCommand::OledReset, Apex2022DisplayCommand::Oled);
    let (OledAction::Show { dry_run, .. } | OledAction::Reset { dry_run }) = action;

    if dry_run {
        let mut buf = vec![0u8; PROFILE_CHUNK_REPORT_SIZE];
        let size = HidReportBuilder::new(HidDeviceType::Keyboard).build_report(command, &mut buf)?;
        println!("dry run, nothing sent: Apex Pro TKL 2023 (0x1628) report [EXPERIMENTAL]");
        print_report(&buf[..size]);
        return Ok(());
    }

    let keyboard_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| Error::Other("No keyboard found".to_string()))?;
    let mut keyboard = manager.open_keyboard(keyboard_info)?;
    crate::warn_if_gg_engine_running();
    keyboard.send_display_command(command)?;
    println!("Sent [EXPERIMENTAL], never confirmed on hardware");
    Ok(())
}

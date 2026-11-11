//! `[EXPERIMENTAL]` Apex Pro TKL 2023 (`0x1628`) live key mappings, macro data and device queries, from GG's
//! decrypted device spec (`base_apex_2022_wired`). GG sends `F2`, `F5` and `90 00` at startup (2026-11-10 USBPcap,
//! each answered on the input endpoint) but not `36`, `B6`, `B7` or `35` for a binding change: it rewrites the
//! on-board profile instead. Those commands are untested from `ssgg`.

use std::collections::HashMap;

use steelseries_gg::devices::hid_reports::{
    DeviceQueryCommand, HidReportBuilder, KEYBOARD_REPORT_SIZE, KeyBinding, MetaToggleCommand, RegionWriteCommand,
};
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::keyboards::key_action::{
    KeyAction, default_key_mappings, key_mapping_read_request, key_mapping_reports, merge_key_mappings,
};
use steelseries_gg::devices::{DeviceManager, DeviceType, HidDeviceType, KeyMappingDatabase, product_ids};
use steelseries_gg::fs_utils::secure_write_async;
use steelseries_gg::{Error, Result};

use super::apex_2023::print_report;
use crate::cli::{DeviceAction, KeymapAction, parse_key_name};

fn open_keyboard(manager: &DeviceManager) -> Result<Box<dyn Keyboard>> {
    let info = manager
        .devices_by_type(DeviceType::Keyboard)
        .into_iter()
        .find(|info| info.product_id == product_ids::APEX_PRO_TKL_2023)
        .ok_or_else(|| Error::DeviceNotFound("no wired Apex Pro TKL 2023 (0x1628) connected".to_string()))?;
    let keyboard = manager.open_keyboard(info)?;
    crate::warn_if_gg_engine_running();
    println!(
        "using {} [EXPERIMENTAL: from GG's device spec, untested from ssgg]",
        info.name
    );
    Ok(keyboard)
}

/// HID usage of a key name, or of `0x<hex>` taken as it is.
fn parse_hid(text: &str) -> Result<u8> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).map_err(|_| Error::InvalidConfig(format!("{text}: not a hex HID usage")));
    }
    let key = parse_key_name(text).ok_or_else(|| Error::InvalidConfig(format!("unknown key name {text}")))?;
    KeyMappingDatabase::new()
        .get_mapping(product_ids::APEX_PRO_TKL_2023)
        .and_then(|mapping| mapping.get_key_address(key))
        .map(|address| address.hid_code)
        .ok_or_else(|| Error::InvalidConfig(format!("{text} is not a key of the Apex Pro TKL 2023")))
}

/// `key:<name>` resolves the name here; every other action (and `key:0x..`) is [`KeyAction::parse`]'s grammar.
fn parse_action(text: &str) -> Result<KeyAction> {
    match text.strip_prefix("key:") {
        Some(name) if !name.starts_with("0x") && !name.starts_with("0X") => Ok(KeyAction::Key(parse_hid(name)?)),
        _ => KeyAction::parse(text),
    }
}

fn parse_change(pair: &str) -> Result<KeyBinding> {
    let (key, action) = pair
        .split_once('=')
        .ok_or_else(|| Error::InvalidConfig(format!("{pair}: expected KEY=ACTION, e.g. CAPSLOCK=key:ESC")))?;
    let (function, codes) = parse_action(action)?.to_bytes();
    Ok((parse_hid(key)?, function, codes))
}

/// Key names by HID usage, from the board's key mapping; usages it does not know print as hex.
fn key_names() -> HashMap<u8, String> {
    let database = KeyMappingDatabase::new();
    let mut names = HashMap::new();
    if let Some(mapping) = database.get_mapping(product_ids::APEX_PRO_TKL_2023) {
        for &key in mapping.get_all_keys() {
            if let Some(address) = mapping.get_key_address(key) {
                names.entry(address.hid_code).or_insert_with(|| key.to_string());
            }
        }
    }
    names
}

fn print_frames(reports: &[Vec<u8>]) {
    for report in reports {
        print_report(report);
    }
}

pub(crate) async fn cmd_keymap(manager: &DeviceManager, action: KeymapAction) -> Result<()> {
    match action {
        KeymapAction::Read { layer } => {
            let bindings = open_keyboard(manager)?.read_key_mappings(layer)?;
            let names = key_names();
            println!("{layer} layer, {} keys:", bindings.len());
            for (hid, function, codes) in bindings {
                let name = names.get(&hid).cloned().unwrap_or_else(|| format!("{hid:#04x}"));
                println!("{name} = {}", KeyAction::from_bytes(function, codes));
            }
        }
        KeymapAction::Set { keys, layer, dry_run } => {
            let changes = keys.iter().map(|pair| parse_change(pair)).collect::<Result<Vec<_>>>()?;
            if dry_run {
                println!("dry run, nothing sent [EXPERIMENTAL]. A live run first reads the {layer} layer:");
                print_report(&key_mapping_read_request(layer)?);
                // The board's own bindings are only known live; the reset state stands in for them here.
                let merged = merge_key_mappings(layer, &default_key_mappings(layer), &changes)?;
                let reports = key_mapping_reports(layer, &merged)?;
                println!(
                    "then writes {} feature report(s), 13 ms apart, shown here on the reset state with your \
                     changes merged:",
                    reports.len()
                );
                print_frames(&reports);
                return Ok(());
            }
            open_keyboard(manager)?.set_key_mappings(layer, &changes)?;
            println!(
                "Sent {} change(s) to the {layer} layer. Live writes are volatile per the spec: read it back with \
                 `ssgg keymap read --layer {layer}`",
                changes.len()
            );
        }
        KeymapAction::Reset { layer, dry_run } => {
            if dry_run {
                let reports = key_mapping_reports(layer, &default_key_mappings(layer))?;
                println!(
                    "dry run, nothing sent [EXPERIMENTAL]: {} feature report(s), 13 ms apart",
                    reports.len()
                );
                print_frames(&reports);
                return Ok(());
            }
            open_keyboard(manager)?.reset_key_mappings(layer)?;
            println!("Sent the reset state of the {layer} layer");
        }
        KeymapAction::Macros { out } => {
            let data = open_keyboard(manager)?.read_macro_data()?;
            println!("{} bytes of macro data read", data.len());
            secure_write_async(out.clone(), data).await?;
            println!("written to {}", out.display());
        }
    }
    Ok(())
}

fn layout_name(id: u8) -> &'static str {
    DeviceQueryCommand::LAYOUTS
        .iter()
        .find(|&&(known, _)| known == id)
        .map_or("not named in GG's spec", |&(_, name)| name)
}

fn region_name(id: u8) -> &'static str {
    match id {
        // The spec's init derives a region from the layout when the board reports 0 or 255.
        0 | 255 => "unset",
        _ => RegionWriteCommand::REGIONS
            .iter()
            .find(|&&(known, _)| known == id)
            .map_or("not named in GG's spec", |&(_, name)| name),
    }
}

pub(crate) fn cmd_device(manager: &DeviceManager, action: DeviceAction) -> Result<()> {
    match action {
        DeviceAction::Info => {
            let mut keyboard = open_keyboard(manager)?;
            match keyboard.firmware_version()? {
                Some(version) => println!("firmware: {version}"),
                None => println!("firmware: no answer"),
            }
            match keyboard.device_layout() {
                Ok(id) => println!("layout: {id} ({})", layout_name(id)),
                Err(e) => println!("layout: {e}"),
            }
            match keyboard.device_region() {
                Ok(id) => println!("region: {id} ({})", region_name(id)),
                Err(e) => println!("region: {e}"),
            }
        }
        DeviceAction::MetaKey { key, dry_run } => {
            let hid = parse_hid(&key)?;
            if dry_run {
                let mut buf = vec![0u8; KEYBOARD_REPORT_SIZE];
                let size =
                    HidReportBuilder::new(HidDeviceType::Keyboard).build_report(MetaToggleCommand { hid }, &mut buf)?;
                println!("dry run, nothing sent [EXPERIMENTAL]: output report");
                print_report(&buf[..size]);
                return Ok(());
            }
            open_keyboard(manager)?.meta_key(hid)?;
            println!(
                "Sent: SteelSeries key assigned to usage {hid:#04x}. Whether it survives a power cycle is unknown"
            );
        }
        DeviceAction::Region { id, yes, dry_run } => {
            if dry_run {
                let mut buf = vec![0u8; KEYBOARD_REPORT_SIZE];
                let size = HidReportBuilder::new(HidDeviceType::Keyboard)
                    .build_report(RegionWriteCommand { region: id }, &mut buf)?;
                println!("dry run, nothing sent [EXPERIMENTAL]: output report (writes flash)");
                print_report(&buf[..size]);
                return Ok(());
            }
            if !yes {
                return Err(Error::InvalidConfig(
                    "the region write is persistent (it writes flash): pass --yes".to_string(),
                ));
            }
            open_keyboard(manager)?.write_region(id)?;
            println!("Sent: region {id} ({})", region_name(id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use steelseries_gg::devices::hid_reports::PROFILE_CHUNK_REPORT_SIZE;
    use steelseries_gg::devices::keyboards::key_action::MappingLayer;

    use super::*;

    #[test]
    fn changes_parse_names_hex_and_actions() {
        // A is HID usage 0x04 on this board.
        assert_eq!(parse_change("A=key:B").unwrap(), (0x04, 0x51, [0x05, 0, 0, 0]));
        assert_eq!(parse_change("0x04=key:0x1D").unwrap(), (0x04, 0x51, [0x1D, 0, 0, 0]));
        assert_eq!(parse_change("0x04=unbound").unwrap(), (0x04, 0x00, [0; 4]));
        assert_eq!(parse_change("0xF0=meta").unwrap(), (0xF0, 0x62, [0; 4]));
        for bad in ["A", "A=", "nokey=unbound", "A=key:nokey", "0xZZ=unbound", "A=mouse:9"] {
            assert!(parse_change(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn layout_and_region_ids_are_named() {
        assert_eq!(layout_name(0), "US");
        assert_eq!(layout_name(2), "not named in GG's spec");
        assert_eq!(region_name(13), "japan");
        assert_eq!(region_name(255), "unset");
    }

    #[test]
    fn dry_run_frames_cover_every_layer() {
        for layer in [MappingLayer::Base, MappingLayer::Meta, MappingLayer::Second] {
            let reports = key_mapping_reports(layer, &default_key_mappings(layer)).unwrap();
            assert!(reports.iter().all(|r| r.len() == PROFILE_CHUNK_REPORT_SIZE), "{layer}");
        }
    }
}

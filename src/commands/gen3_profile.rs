//! `[EXPERIMENTAL]` `ssgg onboard gen3-*`: decode, encode, gated write, activate and restore of the on-board
//! profiles of the wired Apex Pro TKL 2023 (`0x1628`, firmware 1.19.7). The frames are GG's, captured
//! 2026-11-10; none has been replayed from `ssgg` on hardware yet.
//!
//! A write never runs on its own: it first reads all five slots and saves them, then refuses unless the
//! target slot's read blob re-encodes byte for byte through the codec, and unless `--yes` was given.

use std::path::{Path, PathBuf};

use steelseries_gg::config::Config;
use steelseries_gg::devices::hid_reports::{
    Gen3ProfileReadCommand, HidReportBuilder, LoadProfileCommand, PROFILE_CHUNK_REPORT_SIZE, ProfileCommitCommand,
    ProfileEraseCommand, SetProfileVolatileCommand,
};
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::keyboards::apex_pro_tkl_2023::Gen3Reply;
use steelseries_gg::devices::keyboards::apex_pro_tkl_2023_profile::{PROFILE_LEN, profile_chunks};
use steelseries_gg::devices::keyboards::apex_pro_tkl_2023_profile_codec::{
    Actuation, DEVICE_KEY_HID, Gen3Profile, check_lossless,
};
use steelseries_gg::devices::keyboards::key_action::KeyAction;
use steelseries_gg::devices::{DeviceManager, DeviceType, HidDeviceType, product_ids};
use steelseries_gg::fs_utils::secure_write;
use steelseries_gg::{Error, Result};

use crate::cli::{
    OnboardGen3ActivateArgs, OnboardGen3DecodeArgs, OnboardGen3EncodeArgs, OnboardGen3RestoreArgs, OnboardGen3WriteArgs,
};

/// Slots the keyboard stores.
const SLOTS: u8 = Gen3ProfileReadCommand::SLOTS;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| Error::Other(format!("read {}: {e}", path.display())))
}

fn require_yes(yes: bool, what: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    Err(Error::InvalidConfig(format!(
        "{what} changes what the keyboard stores or runs; nothing was sent. Pass --yes to authorise it"
    )))
}

/// The bytes to write: a blob (it must decode with a matching checksum) or a JSON decode report (encoded and
/// sealed here).
fn load_blob(path: &Path) -> Result<Box<[u8; PROFILE_LEN]>> {
    let bytes = read_file(path)?;
    let is_json = bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{');
    let blob = if is_json {
        let text = String::from_utf8(bytes).map_err(|e| Error::Profile(format!("{}: {e}", path.display())))?;
        Gen3Profile::from_json(&text)?.encode()?
    } else {
        Gen3Profile::decode(&bytes).map_err(|e| Error::Profile(format!("{}: {e}", path.display())))?;
        <[u8; PROFILE_LEN]>::try_from(bytes.as_slice())
            .map_err(|_| Error::Profile(format!("{}: not {PROFILE_LEN} bytes", path.display())))?
    };
    Ok(Box::new(blob))
}

/// `<config dir>/gen3-backups/<label>-<unix seconds>` unless a directory was given.
fn backup_dir(given: Option<&Path>, label: &str) -> Result<PathBuf> {
    if let Some(dir) = given {
        return Ok(dir.to_path_buf());
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let base = Config::config_dir()
        .ok_or_else(|| Error::Other("no config directory for the backup; pass --backup-dir".to_string()))?;
    Ok(base.join("gen3-backups").join(format!("{label}-{secs}")))
}

/// The plan `--dry-run` prints, and the header of a real write. Opens no keyboard.
fn plan_lines(slot: u8, blob: &[u8; PROFILE_LEN], backup: &Path, activate: bool) -> Result<Vec<String>> {
    let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
    let mut buf = vec![0u8; PROFILE_CHUNK_REPORT_SIZE];
    let size = builder.build_report(ProfileEraseCommand { slot }, &mut buf)?;
    let erase = hex(&buf[..size.min(4)]);
    let size = builder.build_report(ProfileCommitCommand { slot }, &mut buf)?;
    let commit = hex(&buf[..size.min(3)]);
    let fs_id = Gen3ProfileReadCommand::FIRST_PROFILE_FS + slot;
    let chunks: Vec<_> = profile_chunks(blob, fs_id).collect();
    let first = chunks
        .first()
        .cloned()
        .ok_or_else(|| Error::Profile("the blob yields no chunks".to_string()))?;
    let size = builder.build_report(first, &mut buf)?;
    let first = hex(&buf[..size.min(10)]);

    let mut lines = vec![
        format!("[EXPERIMENTAL] plan for profile slot {slot}, nothing is sent while planning"),
        format!(
            "before anything is written: read all {SLOTS} slots, save them as slot<N>.bin in {}",
            backup.display()
        ),
        format!("guard: the read blob of slot {slot} must re-encode byte for byte, else the write is refused"),
        format!("1 erase (output report) {erase}, then wait up to 5000 ms for an input report"),
        format!(
            "{} chunks (feature reports, {} ms apart), first header {first}",
            chunks.len(),
            Gen3ProfileReadCommand::COMMAND_GAP_MS
        ),
        format!("1 commit (output report) {commit}, then wait up to 5000 ms for an input report"),
        "then read the slot back and compare it with what was written".to_string(),
    ];
    let mut frames = 1 + chunks.len() + 1;
    if activate {
        let size = builder.build_report(LoadProfileCommand { slot }, &mut buf)?;
        let load = hex(&buf[..size.min(3)]);
        let size = builder.build_report(SetProfileVolatileCommand { volatile: false }, &mut buf)?;
        let volatile = hex(&buf[..size.min(3)]);
        lines.push(format!(
            "activate: load {load} (wait for an input report), then {volatile}"
        ));
        frames += 2;
    } else {
        lines.push("no activation (pass --activate, or run gen3-activate later)".to_string());
    }
    lines.push(format!("{frames} frames in all"));
    Ok(lines)
}

/// The keyboard this family's commands run on: the first wired Apex Pro TKL 2023.
fn open_wired_2023(manager: Option<&mut DeviceManager>) -> Result<(Box<dyn Keyboard>, String)> {
    let Some(manager) = manager else {
        return Err(Error::Other("the Gen3 profile command needs a device".to_string()));
    };
    let info = manager
        .devices_by_type(DeviceType::Keyboard)
        .into_iter()
        .find(|info| info.product_id == product_ids::APEX_PRO_TKL_2023)
        .ok_or_else(|| Error::DeviceNotFound("no wired Apex Pro TKL 2023 (0x1628) connected".to_string()))?;
    let name = info.name.to_string();
    let keyboard = manager.open_keyboard(info)?;
    crate::warn_if_gg_engine_running();
    Ok((keyboard, name))
}

/// Read all five slots and save each as `<dir>/slot<N>.bin` before anything is written.
fn backup_all(keyboard: &mut dyn Keyboard, dir: &Path) -> Result<Vec<Vec<u8>>> {
    std::fs::create_dir_all(dir).map_err(|e| Error::Other(format!("create {}: {e}", dir.display())))?;
    let mut reads = Vec::with_capacity(usize::from(SLOTS));
    for slot in 0..SLOTS {
        let read = keyboard.read_gen3_profile(slot)?;
        if !read.checksum_ok() {
            return Err(Error::Profile(format!(
                "slot {slot} read back with a wrong checksum; nothing was written"
            )));
        }
        let path = dir.join(format!("slot{slot}.bin"));
        secure_write(&path, &read.data)?;
        println!(
            "slot {slot} read ({} bytes) and saved to {}",
            read.data.len(),
            path.display()
        );
        reads.push(read.data);
    }
    Ok(reads)
}

/// Write gate (b): the slot about to be replaced must be a blob the codec reproduces exactly.
fn check_target_is_lossless(reads: &[Vec<u8>], slot: u8) -> Result<()> {
    let read = reads
        .get(usize::from(slot))
        .ok_or_else(|| Error::Profile(format!("slot {slot} was not read")))?;
    check_lossless(read)
        .map(|_| ())
        .map_err(|e| Error::Profile(format!("slot {slot}: {e}; nothing was written")))
}

fn print_replies(replies: &[Gen3Reply]) {
    for (step, reply) in replies {
        match reply {
            Some(bytes) => println!("  {step}: answered {}", hex(&bytes[..bytes.len().min(16)])),
            None => println!("  {step}: no answer"),
        }
    }
}

/// Write one slot on an open keyboard, then read it back and compare. The caller has made the backup and
/// passed the guards.
fn write_and_verify(keyboard: &mut dyn Keyboard, slot: u8, blob: &[u8; PROFILE_LEN], activate: bool) -> Result<()> {
    let replies = keyboard.write_gen3_profile(slot, blob)?;
    print_replies(&replies);
    let back = keyboard.read_gen3_profile(slot)?;
    if let Some(at) = back.data.iter().zip(blob.iter()).position(|(a, b)| a != b) {
        return Err(Error::DeviceCommunication(format!(
            "slot {slot} read back differs from what was written at offset {at}; the slot was NOT activated, \
             restore with gen3-restore"
        )));
    }
    if back.data.len() != PROFILE_LEN {
        return Err(Error::DeviceCommunication(format!(
            "slot {slot} read back {} bytes, expected {PROFILE_LEN}",
            back.data.len()
        )));
    }
    println!("slot {slot} written and read back identical");
    if activate {
        print_replies(&keyboard.activate_gen3_profile(slot)?);
        println!("slot {slot} activated");
    }
    Ok(())
}

pub(crate) fn cmd_gen3_decode(args: &OnboardGen3DecodeArgs) -> Result<()> {
    let bytes = read_file(&args.blob)?;
    let (profile, crc_ok) = if args.ignore_crc {
        Gen3Profile::decode_forensic(&bytes)?
    } else {
        (Gen3Profile::decode(&bytes)?, true)
    };
    if !crc_ok {
        eprintln!("warning: the checksum at offset 12320 does not match this blob");
    }
    if args.json {
        println!("{}", profile.to_json()?);
        return Ok(());
    }
    for line in summary_lines(&profile, crc_ok) {
        println!("{line}");
    }
    Ok(())
}

fn summary_lines(profile: &Gen3Profile, crc_ok: bool) -> Vec<String> {
    let custom = |bindings: &[steelseries_gg::devices::keyboards::apex_pro_tkl_2023_profile_codec::Binding]| {
        bindings
            .iter()
            .filter(|b| b.hid != 0 && b.key != KeyAction::Key(b.hid))
            .count()
    };
    let mut distances: Vec<String> = profile.actuation.iter().map(Actuation::to_string).collect();
    distances.sort();
    distances.dedup();
    vec![
        format!(
            "name: {:?}, schema {}/{}",
            profile.name, profile.schema, profile.device_profile_schema
        ),
        format!("checksum: {}", if crc_ok { "matches" } else { "does NOT match" }),
        format!(
            "mapping tables: {} keys differ from identity (base), {} (meta layer), {} entries",
            custom(&profile.button_mappings),
            custom(&profile.button_meta_mappings),
            DEVICE_KEY_HID.len()
        ),
        format!("actuation values in use: {}", distances.join(", ")),
        format!(
            "rapid tap: {}, {} pairs set",
            if profile.rapid_tap_enable == 0 { "off" } else { "on" },
            profile.rapid_tap.iter().filter(|p| p.hid1 != 0 || p.hid2 != 0).count()
        ),
        format!(
            "meta key {:#04x}, GUID {}",
            profile.meta_key_phys_hid,
            hex(&profile.guid.0)
        ),
    ]
}

pub(crate) fn cmd_gen3_encode(args: &OnboardGen3EncodeArgs) -> Result<()> {
    let text = String::from_utf8(read_file(&args.json)?)
        .map_err(|e| Error::Profile(format!("{}: {e}", args.json.display())))?;
    let blob = Gen3Profile::from_json(&text)?.encode()?;
    secure_write(&args.out, blob)?;
    println!(
        "{PROFILE_LEN}-byte blob with a fresh checksum written to {}",
        args.out.display()
    );
    Ok(())
}

pub(crate) fn cmd_gen3_write(manager: Option<&mut DeviceManager>, args: OnboardGen3WriteArgs) -> Result<()> {
    let blob = load_blob(&args.from)?;
    let backup = backup_dir(args.backup_dir.as_deref(), &args.slot.to_string())?;
    if args.dry_run {
        for line in plan_lines(args.slot, &blob, &backup, args.activate)? {
            println!("{line}");
        }
        return Ok(());
    }
    require_yes(args.yes, "gen3-write")?;

    let (mut keyboard, name) = open_wired_2023(manager)?;
    println!("writing slot {} on {name}", args.slot);
    let reads = backup_all(keyboard.as_mut(), &backup)?;
    check_target_is_lossless(&reads, args.slot)?;
    write_and_verify(keyboard.as_mut(), args.slot, &blob, args.activate)
}

pub(crate) fn cmd_gen3_activate(manager: Option<&mut DeviceManager>, args: &OnboardGen3ActivateArgs) -> Result<()> {
    require_yes(args.yes, "gen3-activate")?;
    let (mut keyboard, name) = open_wired_2023(manager)?;
    print_replies(&keyboard.activate_gen3_profile(args.slot)?);
    println!("slot {} activated on {name}", args.slot);
    Ok(())
}

pub(crate) fn cmd_gen3_restore(manager: Option<&mut DeviceManager>, args: OnboardGen3RestoreArgs) -> Result<()> {
    let slots: Vec<u8> = match args.slot {
        Some(slot) => vec![slot],
        None => (0..SLOTS)
            .filter(|slot| args.from.join(format!("slot{slot}.bin")).is_file())
            .collect(),
    };
    if slots.is_empty() {
        return Err(Error::InvalidConfig(format!(
            "no slot<N>.bin in {}",
            args.from.display()
        )));
    }
    let mut blobs = Vec::with_capacity(slots.len());
    for &slot in &slots {
        blobs.push((slot, load_blob(&args.from.join(format!("slot{slot}.bin")))?));
    }
    let backup = backup_dir(args.backup_dir.as_deref(), "restore")?;
    if args.dry_run {
        for (slot, blob) in &blobs {
            for line in plan_lines(*slot, blob, &backup, false)? {
                println!("{line}");
            }
        }
        return Ok(());
    }
    require_yes(args.yes, "gen3-restore")?;

    let (mut keyboard, name) = open_wired_2023(manager)?;
    println!("restoring slots {slots:?} on {name}");
    let reads = backup_all(keyboard.as_mut(), &backup)?;
    for (slot, _) in &blobs {
        check_target_is_lossless(&reads, *slot)?;
    }
    for (slot, blob) in &blobs {
        write_and_verify(keyboard.as_mut(), *slot, blob, false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use steelseries_gg::devices::keyboards::apex_pro_tkl_2023_profile::seal_profile;

    /// A sealed blob with both schema words set and everything else zero.
    fn sealed_blob() -> [u8; PROFILE_LEN] {
        let mut blob = [0u8; PROFILE_LEN];
        blob[0] = 3;
        blob[1080] = 8;
        blob[12_324..].fill(0xFF);
        seal_profile(&mut blob);
        blob
    }

    #[test]
    fn writes_are_refused_without_yes() {
        assert!(require_yes(false, "gen3-write").is_err());
        assert!(require_yes(true, "gen3-write").is_ok());
    }

    #[test]
    fn plan_names_frame_counts_first_header_and_backup() {
        let blob = sealed_blob();
        let plan = plan_lines(2, &blob, Path::new("backups/2-1"), false)
            .unwrap()
            .join("\n");
        assert!(plan.contains("1 erase (output report) 00 02 01 82"), "{plan}");
        assert!(plan.contains("25 chunks"), "{plan}");
        assert!(plan.contains("first header 00 03 01 82 f4 01 00 00 00 00"), "{plan}");
        assert!(plan.contains("1 commit (output report) 00 b1 02"), "{plan}");
        assert!(plan.contains("27 frames in all"), "{plan}");
        assert!(plan.contains("backups/2-1"), "{plan}");
        assert!(plan.contains("no activation"), "{plan}");

        let activated = plan_lines(2, &blob, Path::new("b"), true).unwrap().join("\n");
        assert!(activated.contains("load 00 b2 02"), "{activated}");
        assert!(activated.contains("00 34 00"), "{activated}");
        assert!(activated.contains("29 frames in all"), "{activated}");
    }

    #[test]
    fn load_blob_takes_blobs_and_json_and_refuses_bad_checksums() {
        let dir = tempfile::tempdir().unwrap();
        let blob = sealed_blob();

        let bin = dir.path().join("slot.bin");
        std::fs::write(&bin, blob).unwrap();
        assert_eq!(load_blob(&bin).unwrap()[..], blob[..]);

        let json = dir.path().join("slot.json");
        std::fs::write(&json, Gen3Profile::decode(&blob).unwrap().to_json().unwrap()).unwrap();
        assert_eq!(load_blob(&json).unwrap()[..], blob[..]);

        let mut bad = blob;
        bad[2000] ^= 1;
        let bad_path = dir.path().join("bad.bin");
        std::fs::write(&bad_path, bad).unwrap();
        assert!(load_blob(&bad_path).is_err());
    }

    #[test]
    fn target_slot_must_reencode_exactly() {
        let good = sealed_blob().to_vec();
        let mut lossy = sealed_blob();
        lossy[4 + 10] = b'x'; // name byte behind the NUL: the codec cannot keep it
        seal_profile(&mut lossy);

        let reads = vec![good.clone(), lossy.to_vec(), good.clone(), good.clone(), good];
        assert!(check_target_is_lossless(&reads, 0).is_ok());
        assert!(check_target_is_lossless(&reads, 1).is_err());
        assert!(check_target_is_lossless(&reads[..2], 4).is_err(), "an unread slot");
    }
}

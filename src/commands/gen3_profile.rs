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
use steelseries_gg::devices::keyboards::apex_pro_tkl_2023::{Gen3ProfileRead, Gen3Reply};
use steelseries_gg::devices::keyboards::apex_pro_tkl_2023_profile::{PROFILE_LEN, profile_chunks};
use steelseries_gg::devices::keyboards::apex_pro_tkl_2023_profile_codec::{
    Actuation, DEVICE_KEY_HID, Gen3Profile, NAME_LEN, check_lossless, offsets,
};
use steelseries_gg::devices::keyboards::key_action::KeyAction;
use steelseries_gg::devices::{DeviceManager, DeviceType, HidDeviceType, product_ids};
use steelseries_gg::fs_utils::{secure_open, secure_write};
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

/// Create `path`, refusing to touch a file that is already there.
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = secure_open(path, &options).map_err(|e| {
        Error::FileSystemError(format!(
            "{}: {e} (an existing file is never overwritten)",
            path.display()
        ))
    })?;
    file.write_all(bytes)?;
    Ok(())
}

/// A backup directory must be new or empty, so one run can never overwrite an earlier run's backup.
fn ensure_fresh_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries = std::fs::read_dir(dir).map_err(|e| Error::Other(format!("read {}: {e}", dir.display())))?;
    if entries.next().is_some() {
        return Err(Error::FileSystemError(format!(
            "{} already holds files; pass a new or empty --backup-dir. Nothing was sent",
            dir.display()
        )));
    }
    Ok(())
}

/// The SteelSeries processes (GG's engine, client, ...) among `names`.
fn steelseries_processes<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> Vec<String> {
    let mut found: Vec<String> = names
        .into_iter()
        .map(|n| n.as_ref().to_string())
        .filter(|n| n.to_ascii_lowercase().starts_with("steelseries"))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// GG rewrites the board about 19 times a second and would race a profile write, so the write commands refuse
/// while any SteelSeries process runs.
fn refuse_while_gg_runs(names: &[String]) -> Result<()> {
    let running = steelseries_processes(names);
    if running.is_empty() {
        return Ok(());
    }
    Err(Error::InvalidConfig(format!(
        "SteelSeries GG is running ({}); it rewrites the keyboard and would race this command. Close GG \
         (including its tray icon) and run again. Nothing was sent",
        running.join(", ")
    )))
}

/// Names of the running processes. Windows only: GG has no Linux build.
#[cfg(windows)]
fn running_process_names() -> Vec<String> {
    use sysinfo::{ProcessesToUpdate, System};
    let Ok(mut sys) = System::new() else {
        return Vec::new();
    };
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .map(|p| p.name().to_string_lossy().into_owned())
        .collect()
}

#[cfg(not(windows))]
fn running_process_names() -> Vec<String> {
    Vec::new()
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
    refuse_while_gg_runs(&running_process_names())?;
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
    Ok((keyboard, name))
}

/// Read all five slots and save each readable one as `<dir>/slot<N>.bin` (never over an existing file) before
/// anything is written. A slot in `tolerated` may fail to read or fail its checksum (the slot being repaired by
/// `gen3-restore`): it is skipped, a garbled read is kept as `slot<N>.damaged.bin`, and its entry is `None`.
/// Any other slot that does not read cleanly aborts the run.
fn backup_slots(
    read: &mut dyn FnMut(u8) -> Result<Gen3ProfileRead>,
    dir: &Path,
    tolerated: &[u8],
) -> Result<Vec<Option<Vec<u8>>>> {
    std::fs::create_dir_all(dir).map_err(|e| Error::Other(format!("create {}: {e}", dir.display())))?;
    let mut reads = Vec::with_capacity(usize::from(SLOTS));
    for slot in 0..SLOTS {
        match read(slot) {
            Ok(r) if r.data.len() == PROFILE_LEN && r.checksum_ok() => {
                let path = dir.join(format!("slot{slot}.bin"));
                write_new(&path, &r.data)?;
                println!(
                    "slot {slot} read ({} bytes) and saved to {}",
                    r.data.len(),
                    path.display()
                );
                reads.push(Some(r.data));
            }
            Ok(r) if tolerated.contains(&slot) => {
                if r.data.is_empty() {
                    eprintln!("warning: slot {slot} read back empty");
                } else {
                    let path = dir.join(format!("slot{slot}.damaged.bin"));
                    write_new(&path, &r.data)?;
                    eprintln!(
                        "warning: slot {slot} read back damaged ({} bytes, checksum {}); raw read kept in {}",
                        r.data.len(),
                        if r.checksum_ok() { "ok" } else { "wrong" },
                        path.display()
                    );
                }
                reads.push(None);
            }
            Ok(_) => {
                return Err(Error::Profile(format!(
                    "slot {slot} read back with a wrong checksum; nothing was written"
                )));
            }
            Err(e) if tolerated.contains(&slot) => {
                eprintln!("warning: slot {slot} could not be read: {e}");
                reads.push(None);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(reads)
}

/// Write gate (b): the slot about to be replaced must be a blob the codec reproduces exactly.
fn check_target_is_lossless(reads: &[Option<Vec<u8>>], slot: u8) -> Result<()> {
    let read = reads
        .get(usize::from(slot))
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::Profile(format!("slot {slot} was not read; nothing was written")))?;
    check_lossless(read)
        .map(|_| ())
        .map_err(|e| Error::Profile(format!("slot {slot}: {e}; nothing was written")))
}

/// The field-level changes from the stored `old` blob to the `new` one, as printable lines.
fn diff_lines(old: &[u8], new: &[u8]) -> Vec<String> {
    match (Gen3Profile::decode_forensic(old), Gen3Profile::decode_forensic(new)) {
        (Ok((old, _)), Ok((new, _))) => {
            let diff = old.diff(&new);
            if diff.is_empty() {
                vec!["no field differs".to_string()]
            } else {
                diff
            }
        }
        (Err(e), _) | (_, Err(e)) => vec![format!("no diff: {e}")],
    }
}

fn print_diff(slot: u8, old: Option<&[u8]>, new: &[u8]) {
    match old {
        Some(old) => {
            println!("slot {slot}: changes from what the board holds to what will be written:");
            for line in diff_lines(old, new) {
                println!("  {line}");
            }
        }
        None => println!("slot {slot}: the board's current content is not readable, no diff"),
    }
}

/// `--name-only`: the blobs may differ only in the name field and the checksum.
fn only_name_and_crc_differ(old: &[u8], new: &[u8]) -> bool {
    let name = offsets::NAME..offsets::NAME + NAME_LEN;
    let crc = offsets::CRC..offsets::CRC + 4;
    old.len() == new.len()
        && old
            .iter()
            .zip(new)
            .enumerate()
            .all(|(i, (a, b))| a == b || name.contains(&i) || crc.contains(&i))
}

/// The diff and `--name-only` step of a write, between the backup and the first frame.
fn review_change(slot: u8, current: Option<&[u8]>, new: &[u8], name_only: bool) -> Result<()> {
    print_diff(slot, current, new);
    if name_only && !current.is_some_and(|old| only_name_and_crc_differ(old, new)) {
        return Err(Error::InvalidConfig(format!(
            "--name-only: more than the name of slot {slot} differs from what the board holds; nothing was written"
        )));
    }
    Ok(())
}

fn print_replies(replies: &[Gen3Reply]) {
    for (step, reply) in replies {
        match reply {
            Some(bytes) => println!("  {step}: answered {}", hex(&bytes[..bytes.len().min(16)])),
            None => println!("  {step}: no answer"),
        }
    }
}

/// What to tell the user when a write failed after the erase may have been sent.
fn recovery_error(cause: Error, retry_from: &Path, slot: u8) -> Error {
    Error::DeviceCommunication(format!(
        "{cause}\nslot {slot} may be erased or half written. Restore it with: \
         ssgg onboard gen3-restore --from {} --slot {slot} --yes",
        retry_from.display()
    ))
}

/// Write one slot on an open keyboard, then read it back and compare. The caller has made the backup and
/// passed the guards. `retry_from` is the directory a recovery `gen3-restore` takes its `slot<N>.bin` from.
fn write_and_verify(
    keyboard: &mut dyn Keyboard,
    slot: u8,
    blob: &[u8; PROFILE_LEN],
    activate: bool,
    retry_from: &Path,
) -> Result<()> {
    let fail = |cause: Error| recovery_error(cause, retry_from, slot);
    let replies = keyboard.write_gen3_profile(slot, blob).map_err(fail)?;
    print_replies(&replies);
    let back = keyboard.read_gen3_profile(slot).map_err(fail)?;
    if back.data.len() != PROFILE_LEN {
        return Err(fail(Error::DeviceCommunication(format!(
            "slot {slot} read back {} bytes, expected {PROFILE_LEN}",
            back.data.len()
        ))));
    }
    if let Some(at) = back.data.iter().zip(blob.iter()).position(|(a, b)| a != b) {
        return Err(fail(Error::DeviceCommunication(format!(
            "slot {slot} read back differs from what was written at offset {at}; the slot was NOT activated"
        ))));
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

/// `gen3-encode --out` never replaces its own input, and replaces any other existing file only with `--force`.
fn check_encode_target(input: &Path, out: &Path, force: bool) -> Result<()> {
    let same = match (input.canonicalize(), out.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => input == out,
    };
    if same {
        return Err(Error::InvalidConfig(format!(
            "--out {} is the input file; pick another path",
            out.display()
        )));
    }
    if out.exists() && !force {
        return Err(Error::FileSystemError(format!(
            "{} already exists; pass --force to overwrite it",
            out.display()
        )));
    }
    Ok(())
}

pub(crate) fn cmd_gen3_encode(args: &OnboardGen3EncodeArgs) -> Result<()> {
    let text = String::from_utf8(read_file(&args.json)?)
        .map_err(|e| Error::Profile(format!("{}: {e}", args.json.display())))?;
    let blob = Gen3Profile::from_json(&text)?.encode()?;
    check_encode_target(&args.json, &args.out, args.force)?;
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
        match &args.compare {
            Some(path) => {
                let current = read_file(path)?;
                review_change(args.slot, Some(&current), &blob[..], args.name_only)?;
            }
            None if args.name_only => {
                return Err(Error::InvalidConfig(
                    "--name-only with --dry-run needs --compare <blob of the slot's current content>".to_string(),
                ));
            }
            None => println!("no diff: pass --compare <blob of the slot's current content> to print one"),
        }
        return Ok(());
    }
    require_yes(args.yes, "gen3-write")?;
    ensure_fresh_dir(&backup)?;

    let (mut keyboard, name) = open_wired_2023(manager)?;
    println!("writing slot {} on {name}", args.slot);
    let reads = backup_slots(&mut |slot| keyboard.read_gen3_profile(slot), &backup, &[])?;
    check_target_is_lossless(&reads, args.slot)?;
    let current = reads.get(usize::from(args.slot)).and_then(Option::as_deref);
    review_change(args.slot, current, &blob[..], args.name_only)?;
    write_and_verify(keyboard.as_mut(), args.slot, &blob, args.activate, &backup)
}

pub(crate) fn cmd_gen3_activate(manager: Option<&mut DeviceManager>, args: &OnboardGen3ActivateArgs) -> Result<()> {
    require_yes(args.yes, "gen3-activate")?;
    let (mut keyboard, name) = open_wired_2023(manager)?;
    print_replies(&keyboard.activate_gen3_profile(args.slot)?);
    println!("slot {} activated on {name}", args.slot);
    Ok(())
}

/// The `slot<N>.bin` files `gen3-restore` takes, each checked as a restore source: 12,500 bytes, known schema,
/// its own checksum and a byte-exact codec round trip.
fn load_restore_blobs(from: &Path, only: Option<u8>) -> Result<Vec<(u8, Box<[u8; PROFILE_LEN]>)>> {
    let slots: Vec<u8> = match only {
        Some(slot) => vec![slot],
        None => (0..SLOTS)
            .filter(|slot| from.join(format!("slot{slot}.bin")).is_file())
            .collect(),
    };
    if slots.is_empty() {
        return Err(Error::InvalidConfig(format!("no slot<N>.bin in {}", from.display())));
    }
    slots
        .into_iter()
        .map(|slot| {
            let path = from.join(format!("slot{slot}.bin"));
            let blob = load_blob(&path)?;
            check_lossless(&blob[..]).map_err(|e| Error::Profile(format!("{}: {e}", path.display())))?;
            Ok((slot, blob))
        })
        .collect()
}

/// What a multi-slot restore that stopped at `failed` says about the slots it did and did not reach.
fn restore_progress(cause: &Error, all: &[u8], done: &[u8], failed: u8) -> String {
    let not_reached: Vec<u8> = all
        .iter()
        .copied()
        .filter(|s| !done.contains(s) && *s != failed)
        .collect();
    format!(
        "{cause}\nrestore stopped: slots written and read back identical: {done:?}; failed: {failed}; \
         not attempted: {not_reached:?}"
    )
}

pub(crate) fn cmd_gen3_restore(manager: Option<&mut DeviceManager>, args: OnboardGen3RestoreArgs) -> Result<()> {
    let blobs = load_restore_blobs(&args.from, args.slot)?;
    let slots: Vec<u8> = blobs.iter().map(|(slot, _)| *slot).collect();
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
    ensure_fresh_dir(&backup)?;

    let (mut keyboard, name) = open_wired_2023(manager)?;
    println!("restoring slots {slots:?} on {name}");
    // A slot being repaired may be unreadable or fail its checksum: the saved file is validated instead.
    let reads = backup_slots(&mut |slot| keyboard.read_gen3_profile(slot), &backup, &slots)?;
    for (slot, blob) in &blobs {
        let current = reads.get(usize::from(*slot)).and_then(Option::as_deref);
        print_diff(*slot, current, &blob[..]);
    }
    let mut done: Vec<u8> = Vec::new();
    for (slot, blob) in &blobs {
        if let Err(e) = write_and_verify(keyboard.as_mut(), *slot, blob, false, &args.from) {
            return Err(Error::DeviceCommunication(restore_progress(&e, &slots, &done, *slot)));
        }
        done.push(*slot);
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
        let good = Some(sealed_blob().to_vec());
        let mut lossy = sealed_blob();
        lossy[4 + 10] = b'x'; // name byte behind the NUL: the codec cannot keep it
        seal_profile(&mut lossy);

        let reads = vec![good.clone(), Some(lossy.to_vec()), good.clone(), None, good];
        assert!(check_target_is_lossless(&reads, 0).is_ok());
        assert!(check_target_is_lossless(&reads, 1).is_err());
        assert!(check_target_is_lossless(&reads, 3).is_err(), "an unreadable slot");
        assert!(check_target_is_lossless(&reads[..2], 4).is_err(), "an unread slot");
    }

    fn read_of(blob: &[u8]) -> Result<Gen3ProfileRead> {
        Ok(Gen3ProfileRead {
            firmware: None,
            headers: Vec::new(),
            data: blob.to_vec(),
        })
    }

    #[test]
    fn backup_aborts_on_a_bad_slot_unless_it_is_the_one_being_restored() {
        let good = sealed_blob();
        let mut bad = sealed_blob();
        bad[2000] ^= 1; // checksum no longer matches

        let dir = tempfile::tempdir().unwrap();
        let strict = dir.path().join("strict");
        let mut read = |slot: u8| if slot == 2 { read_of(&bad) } else { read_of(&good) };
        assert!(
            backup_slots(&mut read, &strict, &[]).is_err(),
            "write: any bad slot aborts"
        );

        let restore = dir.path().join("restore");
        let reads = backup_slots(&mut read, &restore, &[2]).unwrap();
        assert_eq!(
            reads.iter().map(Option::is_some).collect::<Vec<_>>(),
            [true, true, false, true, true]
        );
        assert!(restore.join("slot0.bin").is_file() && restore.join("slot4.bin").is_file());
        assert!(!restore.join("slot2.bin").exists());
        assert_eq!(std::fs::read(restore.join("slot2.damaged.bin")).unwrap(), bad.to_vec());

        let failing = dir.path().join("failing");
        let mut read = |slot: u8| {
            if slot == 3 {
                Err(Error::DeviceCommunication("no answer".to_string()))
            } else {
                read_of(&good)
            }
        };
        assert!(
            backup_slots(&mut read, &failing, &[0]).is_err(),
            "an untolerated read failure aborts"
        );
        let tolerated = dir.path().join("tolerated");
        let reads = backup_slots(&mut read, &tolerated, &[3]).unwrap();
        assert!(reads[3].is_none() && reads[4].is_some());
    }

    #[test]
    fn restore_sources_are_validated_before_the_keyboard_is_touched() {
        let dir = tempfile::tempdir().unwrap();
        let good = sealed_blob();
        std::fs::write(dir.path().join("slot1.bin"), good).unwrap();
        let found = load_restore_blobs(dir.path(), None).unwrap();
        assert_eq!(found.iter().map(|(slot, _)| *slot).collect::<Vec<_>>(), [1]);

        let mut lossy = sealed_blob();
        lossy[4 + 10] = b'x';
        seal_profile(&mut lossy);
        std::fs::write(dir.path().join("slot2.bin"), lossy).unwrap();
        let mut bad_crc = sealed_blob();
        bad_crc[2000] ^= 1;
        std::fs::write(dir.path().join("slot3.bin"), bad_crc).unwrap();
        std::fs::write(dir.path().join("slot4.bin"), &good[..100]).unwrap();
        assert!(
            load_restore_blobs(dir.path(), Some(2)).is_err(),
            "not a lossless round trip"
        );
        assert!(load_restore_blobs(dir.path(), Some(3)).is_err(), "own checksum wrong");
        assert!(load_restore_blobs(dir.path(), Some(4)).is_err(), "wrong length");
        assert!(load_restore_blobs(dir.path(), Some(0)).is_err(), "missing file");
        assert!(load_restore_blobs(&dir.path().join("none"), None).is_err());
    }

    #[test]
    fn backups_and_encode_output_never_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("slot0.bin");
        write_new(&file, b"first").unwrap();
        assert!(write_new(&file, b"second").is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"first");

        assert!(ensure_fresh_dir(dir.path()).is_err(), "a directory with files");
        assert!(ensure_fresh_dir(&dir.path().join("new")).is_ok());
        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert!(ensure_fresh_dir(&empty).is_ok());

        let json = dir.path().join("in.json");
        let out = dir.path().join("out.bin");
        std::fs::write(&json, "{}").unwrap();
        assert!(check_encode_target(&json, &out, false).is_ok());
        std::fs::write(&out, "x").unwrap();
        assert!(check_encode_target(&json, &out, false).is_err(), "exists, no --force");
        assert!(check_encode_target(&json, &out, true).is_ok());
        assert!(check_encode_target(&json, &json, true).is_err(), "never its own input");
    }

    #[test]
    fn steelseries_processes_are_refused() {
        let names = [
            "explorer.exe",
            "SteelSeriesEngine.exe",
            "steelseriesgg.exe",
            "SteelSeriesEngine.exe",
            "ssgg.exe",
        ];
        assert_eq!(
            steelseries_processes(names),
            ["SteelSeriesEngine.exe", "steelseriesgg.exe"]
        );
        let running: Vec<String> = names.iter().map(ToString::to_string).collect();
        let err = refuse_while_gg_runs(&running).unwrap_err().to_string();
        assert!(
            err.contains("SteelSeriesEngine.exe") && err.contains("Nothing was sent"),
            "{err}"
        );
        assert!(refuse_while_gg_runs(&running[..1]).is_ok());
        assert!(refuse_while_gg_runs(&[]).is_ok());
    }

    #[test]
    fn name_only_guard_allows_only_name_and_checksum() {
        let old = sealed_blob();
        let mut renamed = Gen3Profile::decode(&old).unwrap();
        renamed.name = "renamed".to_string();
        let renamed = renamed.encode().unwrap();
        assert!(only_name_and_crc_differ(&old, &renamed));
        assert!(review_change(0, Some(&old), &renamed, true).is_ok());

        let mut other = Gen3Profile::decode(&renamed).unwrap();
        other.lighting.lock_keys_brightness = 7;
        let other = other.encode().unwrap();
        assert!(!only_name_and_crc_differ(&old, &other));
        assert!(review_change(0, Some(&old), &other, true).is_err());
        assert!(
            review_change(0, Some(&old), &other, false).is_ok(),
            "without the flag a diff is only printed"
        );
        assert!(review_change(0, None, &renamed, true).is_err(), "unreadable slot");
    }

    #[test]
    fn diff_lines_name_each_changed_field() {
        let old = sealed_blob();
        let mut changed = Gen3Profile::decode(&old).unwrap();
        changed.name = "new".to_string();
        changed.lighting.lock_keys_color = [1, 2, 3];
        changed.release_mode[4] = 9;
        changed.oled.0[10] = 0xAA;
        changed.oled.0[11] = 0xBB;
        let lines = diff_lines(&old, &changed.encode().unwrap()).join("\n");
        assert!(lines.contains("name: \"\" -> \"new\""), "{lines}");
        assert!(lines.contains("lock_keys_color: [0, 0, 0] -> [1, 2, 3]"), "{lines}");
        assert!(lines.contains("release_mode[4]: 0 -> 9"), "{lines}");
        assert!(lines.contains("oled: 2 bytes differ at offsets +10..=+11"), "{lines}");
        assert_eq!(diff_lines(&old, &old), ["no field differs"]);
        assert!(diff_lines(&old[..10], &old)[0].starts_with("no diff"));
    }

    #[test]
    fn a_stopped_restore_names_written_failed_and_untouched_slots() {
        let cause = Error::DeviceCommunication("boom".to_string());
        let text = restore_progress(&cause, &[0, 1, 2, 3], &[0, 1], 2);
        assert!(text.contains("boom"), "{text}");
        assert!(
            text.contains("identical: [0, 1]; failed: 2; not attempted: [3]"),
            "{text}"
        );
    }

    #[test]
    fn failures_after_the_erase_name_the_restore_command() {
        let err = recovery_error(
            Error::DeviceCommunication("boom".to_string()),
            Path::new("backups/1"),
            2,
        )
        .to_string();
        assert!(err.contains("boom"), "{err}");
        assert!(
            err.contains("ssgg onboard gen3-restore --from backups/1 --slot 2 --yes"),
            "{err}"
        );
        assert!(err.contains("erased or half written"), "{err}");
    }
}

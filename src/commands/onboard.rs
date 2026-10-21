use steelseries_gg::devices::DeviceManager;
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::keyboards::onboard::OnboardProfileCodec;
use steelseries_gg::fs_utils::secure_write_async;
use steelseries_gg::{Error, Result};

use crate::cli::OnboardAction;

/// On-board profile commands of a keyboard's on-board protocol. The read collects a
/// profile, `decode` and `encode` work on files, `write` and `load` send frames: all of it runs through the
/// interfaces in src/devices/keyboards/onboard.rs and names no keyboard family - which family a device
/// speaks is what `Keyboard::onboard_codec` answers, and which one a file belongs to is what the codec
/// registry answers.
pub(crate) async fn cmd_onboard(mut manager: Option<&mut DeviceManager>, action: OnboardAction) -> Result<()> {
    use steelseries_gg::devices::keyboards::onboard::{self as onboard, codec_for_bytes, codec_for_json};

    match action {
        OnboardAction::Read(args) => {
            let Some(manager) = manager.as_mut() else {
                return Err(Error::Other("the on-board profile read needs a device".to_string()));
            };
            let keyboard = select_onboard_keyboard(manager, args.device, "read")?;
            // The selection above picked a keyboard whose codec answers, so asking again cannot fail.
            let Some(codec) = keyboard.onboard_codec() else {
                return Err(Error::DeviceCommunication(
                    "the selected keyboard answers no on-board profile codec".to_string(),
                ));
            };
            // GG names its configs 1-5; the family codec maps that number to the slot it stores.
            let slot = codec.slot_for_config(args.profile)?;
            let label = onboard_device_label(keyboard.as_ref());
            println!("reading on the {label}");
            println!();

            if args.verify {
                let first = keyboard.read_onboard_profile(slot).await?;
                let second = keyboard.read_onboard_profile(slot).await?;
                for line in codec.image_lines(&first, &label) {
                    println!("{line}");
                }
                for line in codec.image_lines(&second, &label) {
                    println!("{line}");
                }
                match codec.diff(&first, &second) {
                    Some(offset) => {
                        println!("the two reads differ at profile byte {offset}: treat this pairing as unverified")
                    }
                    None => println!("the two reads agree on all {} profile bytes", first.data.len()),
                }
                let json = onboard_json_path(codec, args.out.as_deref(), &first.data, args.profile);
                let mut profile = codec.decode(&first.data)?;
                // A read's JSON names the keyboard it ran on; a file decode names none.
                first.stamp_identity(&mut profile);
                secure_write_async(json.clone(), codec.profile_json(&profile)?).await?;
                println!("JSON decode report of the first read written to {}", json.display());
                write_onboard_report(codec, args.report.as_ref(), &first).await?;
                if let Some(bin) = args.bin.as_ref() {
                    secure_write_async(bin.clone(), first.data.clone()).await?;
                    let second_path = bin.with_extension("second.bin");
                    secure_write_async(second_path.clone(), second.data.clone()).await?;
                    println!(
                        "both profile images written: {} and {}",
                        bin.display(),
                        second_path.display()
                    );
                    write_onboard_extra(bin, &first).await?;
                }
            } else {
                let image = keyboard.read_onboard_profile(slot).await?;
                for line in codec.image_lines(&image, &label) {
                    println!("{line}");
                }
                if args.hex {
                    print!("{}", codec.hex(&image.data, 0, image.data.len()));
                }
                let json = onboard_json_path(codec, args.out.as_deref(), &image.data, args.profile);
                let mut profile = codec.decode(&image.data)?;
                image.stamp_identity(&mut profile);
                secure_write_async(json.clone(), codec.profile_json(&profile)?).await?;
                println!("JSON decode report written to {}", json.display());
                write_onboard_report(codec, args.report.as_ref(), &image).await?;
                if let Some(bin) = args.bin.as_ref() {
                    secure_write_async(bin.clone(), image.data.clone()).await?;
                    println!("profile image written to {}", bin.display());
                    write_onboard_extra(bin, &image).await?;
                }
            }
        }

        OnboardAction::Decode(args) => {
            let data = read_file(&args.file).await?;
            let codec = codec_for_bytes(&data).ok_or_else(no_family)?;
            println!("schema word: {:04X}", onboard::schema_of(&data));
            let profile = codec.decode(&data)?;
            for line in codec.profile_lines(&profile) {
                println!("{line}");
            }
            if args.hex {
                let from = args.hex_from;
                let to = (from + args.hex_len).min(data.len());
                print!("{}", codec.hex(&data, from, to));
            }
            let json = onboard_json_path(codec, args.out.as_deref(), &data, 1);
            secure_write_async(json.clone(), codec.profile_json(&profile)?).await?;
            println!("JSON decode report written to {}", json.display());
        }
        OnboardAction::Encode(args) => {
            let text = read_text(&args.file).await?;
            let codec = codec_for_json(&text).ok_or_else(no_family)?;
            let data = codec
                .encode_json(&text)
                .map_err(|e| Error::Other(format!("encode {}: {e}", args.file.display())))?;
            // Read the bytes back through the codec's decoder, so what is printed is what the file says,
            // not what the JSON claimed.
            let profile = codec.decode(&data)?;
            println!("{}", codec.summary_line(&profile));
            let out = args.out.unwrap_or_else(|| args.file.with_extension("bin"));
            secure_write_async(out.clone(), data.clone()).await?;
            println!(
                "profile bytes written to {} ({} bytes, checksum {:08X})",
                out.display(),
                data.len(),
                profile.crc_computed
            );
            if let Some(reference) = args.check.as_ref() {
                let wanted = read_file(reference).await?;
                for line in codec.check_lines(&data, &wanted, &reference.display().to_string()) {
                    println!("{line}");
                }
            }
            if args.hex {
                let from = args.hex_from;
                let to = (from + args.hex_len).min(data.len());
                print!("{}", codec.hex(&data, from, to));
            }
        }

        OnboardAction::Write(args) => {
            // The payload source: a decode report encoded here, or payload bytes taken as they are. Both
            // paths pick the family from the bytes' own schema word.
            let (file_codec, payload, source) = match args.bin.as_ref() {
                Some(path) => {
                    let bytes = read_file(path).await?;
                    let codec = codec_for_bytes(&bytes).ok_or_else(no_family)?;
                    (codec, bytes, path.display().to_string())
                }
                None => {
                    let path = args.file.as_ref().ok_or_else(|| {
                        Error::InvalidConfig("a write names a profile JSON, or payload bytes with --bin".to_string())
                    })?;
                    let text = read_text(path).await?;
                    let codec = codec_for_json(&text).ok_or_else(no_family)?;
                    let payload = codec
                        .encode_json(&text)
                        .map_err(|e| Error::Other(format!("encode {}: {e}", path.display())))?;
                    (codec, payload, path.display().to_string())
                }
            };

            let slot = file_codec.slot_for_config(args.profile)?;
            let plan = file_codec
                .write_plan(slot, &payload)
                .map_err(|e| Error::Other(format!("frame the write of {source}: {e}")))?;

            if args.dry_run {
                for line in file_codec.write_banner_lines() {
                    println!("{line}");
                }
                println!();
                for line in onboard::write_plan_lines(&plan) {
                    println!("{line}");
                }
                println!();
                println!(
                    "dry run: no keyboard was opened and nothing was sent; the plan of {} payload bytes is {} \
                     frames for slot {} (GG config {})",
                    payload.len(),
                    plan.frames.len(),
                    slot,
                    args.profile
                );
                return Ok(());
            }

            if !args.yes {
                return Err(Error::InvalidConfig(
                    "an on-board profile write rewrites the slot's stored profile: pass --yes to send it, or \
                     --dry-run to print the plan"
                        .to_string(),
                ));
            }

            let Some(manager) = manager.as_mut() else {
                return Err(Error::Other(
                    "an on-board profile write needs a keyboard: --dry-run prints the plan without one".to_string(),
                ));
            };
            let keyboard = select_onboard_keyboard(manager, args.device, "write")?;
            // The keyboard's own codec frames the write: geometry and file id come from the device the write
            // runs on, not from a codec picked off the payload's schema word.
            let codec = keyboard.onboard_codec().ok_or_else(|| {
                Error::DeviceCommunication(
                    "the selected keyboard answers no on-board profile codec: a write needs the keyboard's own \
                     family geometry"
                        .to_string(),
                )
            })?;
            if codec.family_name() != file_codec.family_name() {
                return Err(Error::InvalidConfig(format!(
                    "{} is a profile of {} ({} payload bytes, schema {:04X}); the selected keyboard speaks {} \
                     - write a profile of the keyboard's own family",
                    source,
                    file_codec.family_name(),
                    payload.len(),
                    onboard::schema_of(&payload),
                    codec.family_name()
                )));
            }
            let slot = codec.slot_for_config(args.profile)?;
            for line in codec.write_banner_lines() {
                println!("{line}");
            }
            println!();
            let plan = codec
                .write_plan(slot, &payload)
                .map_err(|e| Error::Other(format!("frame the write of {source}: {e}")))?;
            for line in onboard::write_plan_lines(&plan) {
                println!("{line}");
            }
            println!();

            let label = onboard_device_label(keyboard.as_ref());
            println!(
                "writing on the {label}: this rewrites the profile file of slot {slot} and makes the keyboard \
                 run that slot"
            );
            let stats = keyboard.write_onboard_profile(slot, &payload).await?;
            let planned = codec.write_stats(&plan);
            println!(
                "write finished: {} of {} frames sent, {} of {} payload bytes, {} of {} pushes, {} of {} \
                 answers read back",
                stats.frames_sent,
                planned.frames_sent,
                stats.bytes_written,
                planned.bytes_written,
                stats.pushes,
                planned.pushes,
                stats.acks,
                planned.acks
            );
            if stats != planned {
                println!("the write did not do every frame its plan listed: read the slot back before trusting it");
            }
            println!(
                "slot {slot} (GG config {}) written and deployed on the {label}",
                args.profile
            );
        }

        OnboardAction::Load(args) => {
            let Some(manager) = manager.as_mut() else {
                return Err(Error::Other("the on-board profile deploy needs a keyboard".to_string()));
            };
            let keyboard = select_onboard_keyboard(manager, args.device, "deploy")?;
            let Some(codec) = keyboard.onboard_codec() else {
                return Err(Error::DeviceCommunication(
                    "the selected keyboard answers no on-board profile codec".to_string(),
                ));
            };
            let slot = codec.slot_for_config(args.profile)?;
            let label = onboard_device_label(keyboard.as_ref());
            keyboard.load_onboard_profile(slot).await?;
            println!(
                "slot {slot} (GG config {}) deployed on the {label}: the keyboard now runs it",
                args.profile
            );
        }
    }

    Ok(())
}

/// The error every file-only on-board command answers when the build has no family codec registered.
fn no_family() -> Error {
    Error::Other("no on-board profile family is registered: decode and encode have nothing to read with".to_string())
}

/// Bytes of a profile file: `onboard decode`'s input, `onboard encode --check`'s target.
async fn read_file(path: &std::path::Path) -> Result<Vec<u8>> {
    tokio::fs::read(path)
        .await
        .map_err(|e| Error::Other(format!("read {}: {e}", path.display())))
}

/// Text of a profile JSON: `onboard encode`'s input.
async fn read_text(path: &std::path::Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(|e| Error::Other(format!("read {}: {e}", path.display())))
}

/// The keyboard an on-board command runs on: the selection the device manager makes, turned into the
/// messages the on-board commands answer with when it does not yield exactly one keyboard. `what` names the
/// command in those messages. Read, write and deploy all run on the keyboard's own handle, so all three use
/// this one selection.
fn select_onboard_keyboard(
    manager: &mut steelseries_gg::devices::discovery::DeviceManager,
    device: Option<u16>,
    what: &str,
) -> Result<Box<dyn Keyboard>> {
    use steelseries_gg::devices::discovery::OnboardKeyboard;

    match manager
        .open_onboard_keyboard(device)
        .map_err(|e| Error::Other(format!("select the on-board profile keyboard: {e}")))?
    {
        OnboardKeyboard::Keyboard(keyboard) => Ok(keyboard),
        OnboardKeyboard::NotFound { product_id } => Err(Error::DeviceNotFound(match product_id {
            Some(pid) => format!(
                "no keyboard with product ID {pid:04X} answers the on-board profile protocol: the {what} runs \
                 on a keyboard's own handle, only keyboards are candidates, and the only keyboard that answers \
                 the protocol is the Apex 7 (0x1612), the board it was measured on. `ssgg devices` lists what \
                 was found; a keyboard listed there that cannot be opened is one whose control interface \
                 another program holds"
            ),
            None => format!(
                "no connected keyboard answers the on-board profile protocol: the {what} runs on a keyboard's \
                 own handle, and only keyboards are candidates (a mouse or a headset is not). A keyboard whose \
                 control interface is taken is not a candidate either: GG's engine (Windows), this crate's \
                 daemon, or an `onboard read` running elsewhere can hold it. Stop what holds it and retry"
            ),
        })),
        OnboardKeyboard::Ambiguous { keyboards } => {
            let listed = keyboards
                .iter()
                .map(|(pid, name)| format!("{name} ({pid:04X})"))
                .collect::<Vec<_>>()
                .join(", ");
            let advice = if keyboards.windows(2).all(|pair| pair[0].0 == pair[1].0) {
                "they share one product ID, which --device cannot tell apart: disconnect all but one"
            } else {
                "name one with --device <pid>"
            };
            Err(Error::InvalidConfig(format!(
                "more than one connected keyboard answers the on-board profile protocol: {listed} - {advice}"
            )))
        }
    }
}

/// Where a device-driven read ran: the keyboard's name and the collection the manager opened.
fn onboard_device_label(keyboard: &dyn Keyboard) -> String {
    format!("{} ({})", keyboard.info().name, keyboard.info().path)
}

/// Where an on-board dump is written: `--out` when given, otherwise the file name the family's codec
/// yields from the profile name inside the payload (`Config 1` -> `config_1.json`). `config` is GG's
/// config number, used only when the payload carries no name to build a file name from.
fn onboard_json_path(
    codec: &dyn OnboardProfileCodec,
    given: Option<&std::path::Path>,
    data: &[u8],
    config: u8,
) -> std::path::PathBuf {
    given.map_or_else(
        || std::path::PathBuf::from(codec.dump_name(data, config)),
        std::path::PathBuf::from,
    )
}

/// The region a read image carries behind its payload, written beside the image as `<out>.extra.bin`.
/// The read fills the whole profile region, and only the payload is decoded; this file is the rest of the
/// region, recorded for a read against GG's capture. Nothing in ssgg decodes these bytes.
async fn write_onboard_extra(
    bin: &std::path::Path,
    image: &steelseries_gg::devices::keyboards::onboard::ProfileImage,
) -> Result<()> {
    let Some(region) = steelseries_gg::devices::keyboards::apex_7_profile::extra_region(&image.data) else {
        return Ok(());
    };
    let extra_path = bin.with_extension("extra.bin");
    secure_write_async(extra_path.clone(), image.data[region.offset..].to_vec()).await?;
    println!(
        "the {} bytes behind the payload written to {} (recorded, decoded by nothing)",
        region.len,
        extra_path.display()
    );
    Ok(())
}

/// The per-pull read record as the family codec writes it: chunk headers, framing stats, the image hash.
/// Capture diagnostics for comparing a read with GG's capture; the decoded profile is what `--out` writes.
async fn write_onboard_report(
    codec: &dyn OnboardProfileCodec,
    path: Option<&std::path::PathBuf>,
    image: &steelseries_gg::devices::keyboards::onboard::ProfileImage,
) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    secure_write_async(path.to_path_buf(), codec.image_json(image)?).await?;
    println!("read record written to {}", path.display());
    Ok(())
}

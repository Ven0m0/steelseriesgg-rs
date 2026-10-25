use clap::{Parser, Subcommand};

use std::path::PathBuf;

use steelseries_gg::devices::KeyId;
use steelseries_gg::devices::hid_reports::RapidTapPair;
use steelseries_gg::rgb::Color;
use steelseries_gg::{Error, Result};

#[cfg(feature = "audio")]
use steelseries_gg::audio::Channel;

/// SteelSeries GG for Linux - Control your SteelSeries devices
#[derive(Parser)]
#[command(name = "ssgg")]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    /// Enable debug logging
    #[arg(short, long)]
    pub(crate) debug: bool,

    /// Enable HID communication debugging and diagnostics
    #[arg(long)]
    pub(crate) debug_hid: bool,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// List connected SteelSeries devices
    Devices,

    /// Control RGB lighting
    Rgb {
        #[command(subcommand)]
        action: RgbAction,
    },

    /// Control actuation points on compatible keyboards
    Actuation {
        #[command(subcommand)]
        action: ActuationAction,
    },

    /// Apex Pro TKL 2023 OLED (EXPERIMENTAL and untested on hardware)
    Oled {
        #[command(subcommand)]
        action: OledAction,
    },

    /// Control headsets (Arctis Nova 3/4/5/7, Nova Pro and Arctis 7+ families only, EXPERIMENTAL and untested on hardware)
    Headset {
        #[command(subcommand)]
        action: HeadsetAction,
    },

    /// Control Modern-family mice: Aerox, Prime Mini/Wireless, Rival 3 Gen 2, Rival 5 (EXPERIMENTAL and untested on hardware)
    Mouse {
        #[command(subcommand)]
        action: MouseAction,
    },

    /// Remap keys on a SteelSeries keyboard (Linux, needs the `remap` feature; EXPERIMENTAL and never run on hardware)
    Remap {
        #[command(subcommand)]
        action: RemapAction,
    },

    /// Manage profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Control audio mixer (Sonar replacement)
    #[cfg(feature = "audio")]
    Audio {
        #[command(subcommand)]
        action: AudioAction,
    },

    /// Control SteelSeries Sonar (direct API access)
    #[cfg(feature = "sonar")]
    Sonar {
        #[command(subcommand)]
        action: SonarAction,
    },

    /// Configure USB polling rate (requires sudo)
    Pollrate {
        #[command(subcommand)]
        action: PollrateAction,
    },

    /// Read a keyboard's on-board profile, decode a saved one, encode a decode report back to bytes,
    /// write a profile and deploy a slot (reverse-engineered protocol, hardware-confirmed on the Apex 7;
    /// decode and encode touch files only)
    Onboard {
        #[command(subcommand)]
        action: OnboardAction,
    },

    /// Serve the on-board profile editor (tools/profile-gui) on 127.0.0.1, with a live lighting panel that
    /// drives the first keyboard. `[EXPERIMENTAL]` lighting; only a solid colour on the wired Apex Pro TKL 2023
    /// is hardware-confirmed
    Gui {
        /// Port to listen on (0 picks a free one)
        #[arg(short, long, default_value = "8765")]
        port: u16,
    },

    /// Start the GameSense server
    Server {
        /// Port to listen on
        #[arg(short, long, default_value = "27301")]
        port: u16,
    },

    /// Run validation tests on connected devices
    Validate {
        /// Enable performance benchmarks
        #[arg(short, long)]
        benchmark: bool,

        /// Test timeout in seconds
        #[arg(short, long, default_value = "30")]
        timeout: u64,

        /// Export validation report to file
        #[arg(short, long)]
        output: Option<String>,

        /// JSON output format
        #[arg(long)]
        json: bool,
    },

    /// Monitor and control RGB performance optimizations
    Performance {
        #[command(subcommand)]
        action: PerformanceAction,
    },

    /// Generate a comprehensive bug report with diagnostic information
    BugReport {
        /// Output file path
        #[arg(short, long, default_value = "ssgg_bug_report.json")]
        output: String,

        /// Include HID communication logs (if available)
        #[arg(long)]
        include_hid_logs: bool,

        /// Include performance metrics snapshot (not available yet: needs the daemon, currently omitted with a warning)
        #[arg(long)]
        include_performance: bool,
    },

    /// Show real-time device connection status
    Status {
        /// Filter by device type (keyboard, headset, or all)
        #[arg(short, long, default_value = "all")]
        device: String,

        /// Refresh interval in milliseconds
        #[arg(short, long, default_value = "1000")]
        refresh: u64,
    },

    /// View HID communication logs with filtering
    HidLogs {
        /// Enable file logging
        #[arg(short, long)]
        file: bool,

        /// Filter by device type (keyboard, headset, or all)
        #[arg(short, long)]
        device: Option<String>,
    },

    /// Run as a daemon (device control + GameSense server)
    Daemon,

    /// Run automated device tests to verify responsiveness
    TestDevice {
        /// Device name or path to test
        device: String,

        /// Enable performance benchmarks
        #[arg(short, long)]
        benchmark: bool,

        /// Show detailed output including passing tests
        #[arg(short, long)]
        verbose: bool,
    },

    /// Verify RGB performance metrics over time
    VerifyPerformance {
        /// Duration to monitor in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,

        /// Effect to test (breathing, spectrum, wave, etc.)
        #[arg(short, long, default_value = "breathing")]
        effect: String,

        /// Export metrics to JSON file
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Protocol Fuzzer (Developer Tool)
    #[command(hide = true)]
    Fuzz {
        /// Start command byte
        #[arg(short, long, default_value = "0x00", value_parser = parse_hex_u8)]
        start: u8,

        /// End command byte
        #[arg(short, long, default_value = "0xFF", value_parser = parse_hex_u8)]
        end: u8,

        /// Delay between commands in ms
        #[arg(short, long, default_value = "100")]
        delay: u64,
    },
}

/// Read the on-board profile of a keyboard that supports this feature.
///
/// `decode` and `encode` work on files only and never touch a device. `read` talks to a keyboard: the
/// protocol is reverse-engineered, and the read changes nothing on the keyboard. `write` and `load` change
/// what the keyboard stores. All of them are hardware-confirmed on an Apex 7 (`0x1612`).
#[derive(Subcommand)]
pub(crate) enum OnboardAction {
    /// Read one profile from a keyboard that answers the on-board profile protocol. Read-only: it writes
    /// no profile.
    Read(OnboardReadArgs),

    /// Decode a profile image saved by an earlier read (no device, no hardware).
    Decode(OnboardDecodeArgs),

    /// Encode a profile JSON back into profile bytes (no device, no hardware). Nothing is sent to a
    /// keyboard;
    Encode(OnboardEncodeArgs),

    /// Write a profile into a keyboard's on-board slot. **This changes the keyboard**: it rewrites the
    /// slot's profile file and then makes the keyboard run that slot. The frames are hardware-confirmed on
    /// an Apex 7; `--yes` is what authorises sending them.
    Write(OnboardWriteArgs),

    /// Make a keyboard run one of its on-board profile slots: GG's profile deploy frame (0x89). **This
    /// changes what the keyboard is running right now.** Hardware-confirmed on an Apex 7.
    Load(OnboardLoadArgs),

    /// EXPERIMENTAL, untested on hardware: read one profile slot of the wired Apex Pro TKL 2023 (0x1628)
    /// with the chunked read from GG's device spec. Read-only; nothing is written to the keyboard.
    Gen3Read(OnboardGen3ReadArgs),
}

/// A read-only Gen3 on-board profile read.
#[derive(clap::Args)]
pub(crate) struct OnboardGen3ReadArgs {
    /// Profile slot, 0-4 (GG's Config 1 is assumed to be slot 0; unverified)
    #[arg(short, long, default_value = "0")]
    pub(crate) slot: u8,

    /// Where to write the 12,500-byte profile blob
    #[arg(long, value_name = "FILE")]
    pub(crate) bin: Option<PathBuf>,

    /// Print the request frames instead of opening a keyboard
    #[arg(long)]
    pub(crate) dry_run: bool,
}

impl OnboardAction {
    /// Whether this subcommand runs on a keyboard, and so has to enumerate hardware **before** it is
    /// dispatched. File-only commands (`decode`, `encode`) and a write asked for its plan alone
    /// (`--dry-run`) answer false and get no device manager at all. A real write answers true: it sends
    /// frames, so it runs on a keyboard's own handle like a read and a deploy do.
    pub(crate) const fn needs_keyboard(&self) -> bool {
        match self {
            Self::Read(_) | Self::Load(_) => true,
            Self::Write(args) => !args.dry_run,
            Self::Gen3Read(args) => !args.dry_run,
            Self::Decode(_) | Self::Encode(_) => false,
        }
    }
}

/// One on-board profile read. The flags pick the keyboard, the config number and what to do with the
/// result; nothing on the wire is a flag.
#[derive(clap::Args)]
pub(crate) struct OnboardReadArgs {
    /// GG's config number (`Config 1` is `1`). How many configs the keyboard has, and which slot is behind
    /// the number, is what the keyboard's family answers - an out-of-range number is refused there, in the
    /// family's own wording.
    #[arg(short, long, default_value = "1", value_parser = clap::value_parser!(u8))]
    pub(crate) profile: u8,

    /// Product ID of the keyboard to read, as hex. Optional **only** when exactly one connected
    /// keyboard answers the on-board profile protocol - a mouse or a headset is not a candidate, and several
    /// collections of one keyboard count as one. Every SteelSeries device answers the same vendor ID, so the
    /// product ID identifies the device. The read runs on that keyboard's own handle.
    #[arg(short, long, value_name = "PID", value_parser = parse_product_id)]
    pub(crate) device: Option<u16>,

    /// Where to write the decoded profile as JSON - the same report `onboard decode` writes for this
    /// image, and the format `tools/profile-gui` opens. Default: the profile name the read returns,
    /// sanitized (`Config 1` -> `config_1.json`), in the current directory
    #[arg(short, long, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,

    /// Also write the assembled profile image, as bytes. No default: bytes are only wanted with a path
    #[arg(long, value_name = "FILE")]
    pub(crate) bin: Option<PathBuf>,

    /// Also write the read record as JSON: the per-pull chunk headers, framing stats, image hash and the
    /// first bytes of the image. Capture diagnostics, not the profile. No default
    #[arg(long, value_name = "FILE")]
    pub(crate) report: Option<PathBuf>,

    /// Hex-dump the whole profile image to stdout
    #[arg(long)]
    pub(crate) hex: bool,

    /// Read twice and compare the profile bytes
    #[arg(long)]
    pub(crate) verify: bool,
}

/// clap parser for `onboard read --device`: the product ID in the hex form every reference writes it in.
fn parse_product_id(text: &str) -> std::result::Result<u16, String> {
    let text = text.trim();
    let digits = text.strip_prefix("0x").or(text.strip_prefix("0X")).unwrap_or(text);
    u16::from_str_radix(digits, 16).map_err(|_| {
        format!(
            "{text} is not a product ID: write it as the hex number a device reports as its `idProduct` (`1612`, `0x1612`), the form ssgg's on-board notes use"
        )
    })
}

#[derive(clap::Args)]
pub(crate) struct OnboardDecodeArgs {
    /// Profile image file (assembled by `ssgg onboard read --bin`)
    pub(crate) file: PathBuf,

    /// Where to write the JSON decode report. Default: the profile name in the image, sanitized
    /// (`Config 1` -> `config_1.json`), in the current directory
    #[arg(short, long, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,

    /// Hex-dump profile bytes
    #[arg(long)]
    pub(crate) hex: bool,

    /// First byte of the hex dump
    #[arg(long, default_value = "0")]
    pub(crate) hex_from: usize,

    /// Number of bytes in the hex dump
    #[arg(long, default_value = "64")]
    pub(crate) hex_len: usize,
}

/// One profile JSON written back as payload bytes. The JSON is the input, the geometry is the
/// decoder's layout table, and the checksum is computed here: the report's stored word is ignored.
#[derive(clap::Args)]
pub(crate) struct OnboardEncodeArgs {
    /// Profile JSON, as written by `ssgg onboard decode`
    pub(crate) file: PathBuf,

    /// Where to write the profile bytes. Default: the input file name with a `.bin` extension
    #[arg(short, long, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,

    /// Compare the encoded bytes with a profile image (e.g. the `--bin` of the read this JSON came
    /// from) and report where they differ, or that they are identical
    #[arg(long, value_name = "FILE")]
    pub(crate) check: Option<PathBuf>,

    /// Hex-dump the encoded bytes
    #[arg(long)]
    pub(crate) hex: bool,

    /// First byte of the hex dump
    #[arg(long, default_value = "0")]
    pub(crate) hex_from: usize,

    /// Number of bytes in the hex dump
    #[arg(long, default_value = "64")]
    pub(crate) hex_len: usize,
}

/// One on-board profile write. The payload source is a profile JSON (encoded here, checksum computed
/// here) or, with `--bin`, the payload bytes an earlier `ssgg onboard read --bin` wrote. The geometry is the
/// keyboard family's (see
/// [`OnboardProfileCodec::write_plan`](crate::devices::keyboards::onboard::OnboardProfileCodec::write_plan))
/// and the frames are the write capture's. Without `--dry-run` the frames are sent.
#[derive(clap::Args)]
pub(crate) struct OnboardWriteArgs {
    /// Profile JSON as written by `ssgg onboard decode` (or `onboard read --out`): encoded to payload bytes
    /// here. Required unless `--bin` names the payload bytes to write
    #[arg(required_unless_present = "bin")]
    pub(crate) file: Option<PathBuf>,

    /// Write these payload bytes as they are, with no encode step: the `--bin` image of an earlier
    /// `ssgg onboard read`, or bytes `onboard encode` wrote
    #[arg(long, value_name = "FILE", conflicts_with = "file")]
    pub(crate) bin: Option<PathBuf>,

    /// GG's config number to write (`Config 1` is `1`). There is no default: a write names the slot it
    /// rewrites. The keyboard's family maps the number to a slot and refuses a number it does not have.
    #[arg(short, long, value_parser = clap::value_parser!(u8))]
    pub(crate) profile: u8,

    /// Product ID of the keyboard to write, as hex. Optional only when exactly one connected keyboard
    /// answers the on-board profile protocol. The write runs on that keyboard's own handle
    #[arg(short, long, value_name = "PID", value_parser = parse_product_id)]
    pub(crate) device: Option<u16>,

    /// Print the frames a write would send, in wire order, and stop. No keyboard is opened and nothing is
    /// sent
    #[arg(long)]
    pub(crate) dry_run: bool,

    /// Confirm the write. Without it (and without `--dry-run`) nothing is sent: a write rewrites the slot's
    /// stored profile and makes the keyboard run it
    #[arg(long)]
    pub(crate) yes: bool,
}

/// One on-board profile deploy: make the keyboard run the slot GG's config number names.
#[derive(clap::Args)]
pub(crate) struct OnboardLoadArgs {
    /// GG's config number to run (`Config 1` is `1`). There is no default: a deploy names the slot it runs.
    #[arg(short, long, value_parser = clap::value_parser!(u8))]
    pub(crate) profile: u8,

    /// Product ID of the keyboard, as hex. Optional only when exactly one connected keyboard answers
    /// the on-board profile protocol
    #[arg(short, long, value_name = "PID", value_parser = parse_product_id)]
    pub(crate) device: Option<u16>,
}

#[derive(Subcommand)]
pub(crate) enum RgbAction {
    /// Set a static color
    Color {
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Set brightness (0-100)
    Brightness {
        /// Brightness level
        level: u8,
    },

    /// Set a lighting effect
    Effect {
        /// Effect name: static, breathing, spectrum, wave, off
        name: String,

        /// Effect speed (0.1 - 5.0)
        #[arg(short, long, default_value = "1.0")]
        speed: f32,
    },

    /// Turn off all LEDs
    Off,

    /// Per-key RGB control (requires supported keyboard with key mapping)
    Perkey {
        #[command(subcommand)]
        action: PerKeyAction,
    },

    /// Apex Pro TKL 2023 (0x1628): leave direct-mode lighting so the board shows its stored profile again
    /// (`00 41` from GG's device spec; EXPERIMENTAL, untested on hardware)
    Release,
}

/// Apex Pro TKL 2023 (0x1628) OLED, from GG's device spec. EXPERIMENTAL, untested on hardware; the pixel
/// packing of the 640 bytes is not in the spec.
#[derive(Subcommand)]
pub(crate) enum OledAction {
    /// Send a raw 640-byte frame file to the OLED
    Show {
        file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },

    /// Hand the OLED back to the keyboard firmware
    Reset {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum HeadsetAction {
    /// Show battery, connection, charging and ChatMix
    Status,

    /// Set sidetone (0-100, mapped onto the headset's 4 steps)
    Sidetone {
        /// Sidetone level
        level: u8,
    },

    /// Set the inactivity auto-off timeout (0 = disabled)
    AutoOff {
        /// Timeout in minutes
        minutes: u8,
    },

    /// Set microphone volume (0-100, mapped onto the headset's 8 steps)
    MicVolume {
        /// Microphone volume
        level: u8,
    },
}

#[derive(Subcommand)]
pub(crate) enum MouseAction {
    /// Show the battery level of a wireless model
    Status,

    /// Set the DPI (range and step depend on the model, e.g. 100 to 18000 in steps of 100)
    Dpi {
        /// DPI value
        value: u16,
    },

    /// Set the USB polling rate of the mouse itself (not the OS-level `pollrate` command)
    PollRate {
        /// Rate in Hz: 125, 250, 500 or 1000
        hz: u16,
    },

    /// Set every LED zone to one colour
    Color {
        /// Colour name or hex (e.g. red, FF8800)
        color: String,
    },
}

/// The Apex Pro TKL 2023 (0x1628) settings below come from GG's device spec and are EXPERIMENTAL: never
/// confirmed on hardware, and whether they survive unplugging is unknown. Each one sets every analog key.
#[derive(Subcommand)]
pub(crate) enum ActuationAction {
    /// Set actuation point for all keys (global). Apex Pro TKL 2023: 0.1-4.0 mm in 0.1 mm steps
    Set {
        /// Actuation point in millimeters (0.1 to 4.0mm, e.g., 1.2, 2.5, 3.6)
        mm: f32,
        /// Apex Pro TKL 2023: set the second actuation layer instead
        #[arg(long)]
        second: bool,
        /// Print the report instead of sending it (Apex Pro TKL 2023 frame)
        #[arg(long)]
        dry_run: bool,
    },

    /// Set actuation point in 0.1mm increments (e.g., 4 = 0.4mm, 25 = 2.5mm)
    SetValue {
        /// Actuation value in 0.1mm units (1-40)
        value: u8,
        /// Apex Pro TKL 2023: set the second actuation layer instead
        #[arg(long)]
        second: bool,
        /// Print the report instead of sending it (Apex Pro TKL 2023 frame)
        #[arg(long)]
        dry_run: bool,
    },

    /// Apex Pro TKL 2023: set the actuation of single keys, e.g. `set-key W=1.0 A=2.0`. The other keys keep the
    /// values ssgg last sent, so run `set <mm>` once first; changes made in GG since then are not seen
    SetKey {
        /// KEY=MM pairs (key names as in `rgb perkey`; mm 0.1-4.0)
        #[arg(required = true)]
        keys: Vec<String>,
        /// Set the second actuation layer instead
        #[arg(long)]
        second: bool,
        /// Print the report instead of sending it
        #[arg(long)]
        dry_run: bool,
    },

    /// Apex Pro TKL 2023: zero the second actuation layer (the state GG showed as no second actuation)
    ClearSecond {
        #[arg(long)]
        dry_run: bool,
    },

    /// Apex Pro TKL 2023: release mode of every analog key (0 = off; 2 and 3 were seen as Rapid Trigger and
    /// Key Guard in GG's profile; 4 is unknown)
    ReleaseMode {
        #[arg(value_parser = clap::builder::PossibleValuesParser::new(["0", "2", "3", "4"]))]
        mode: String,
        #[arg(long)]
        dry_run: bool,
    },

    /// Apex Pro TKL 2023: rapid-trigger sensitivity of every analog key (raw unit, GG's live value was 2)
    RapidTrigger {
        sensitivity: u8,
        #[arg(long)]
        dry_run: bool,
    },

    /// Apex Pro TKL 2023: rapid tap on or off
    RapidTap {
        #[arg(value_parser = clap::builder::PossibleValuesParser::new(["on", "off"]))]
        state: String,
        #[arg(long)]
        dry_run: bool,
    },

    /// Apex Pro TKL 2023: replace the rapid-tap pair list (none given clears it)
    RapidTapPairs {
        /// Up to 10 pairs as hex HID usages KEY1:KEY2[:MODE[:both]], e.g. 04:07 or 04:07:1:both (mode 0-3)
        #[arg(value_parser = parse_rapid_tap_pair)]
        pairs: Vec<RapidTapPair>,
        #[arg(long)]
        dry_run: bool,
    },
}

/// `KEY1:KEY2[:MODE[:both]]`, hex usages.
fn parse_rapid_tap_pair(text: &str) -> std::result::Result<RapidTapPair, String> {
    let parts: Vec<&str> = text.split(':').collect();
    let (hid_1, hid_2) = match parts.as_slice() {
        [a, b, ..] if parts.len() <= 4 => (parse_hex_u8(a)?, parse_hex_u8(b)?),
        _ => return Err(format!("{text}: expected KEY1:KEY2[:MODE[:both]]")),
    };
    let mode = parts
        .get(2)
        .map_or(Ok(0), |m| m.parse::<u8>().map_err(|e| format!("mode: {e}")))?;
    let report_both = match parts.get(3) {
        None => false,
        Some(flag) if flag.eq_ignore_ascii_case("both") => true,
        Some(flag) => return Err(format!("{flag}: the fourth field can only be `both`")),
    };
    Ok(RapidTapPair {
        hid_1,
        hid_2,
        mode,
        report_both,
    })
}

#[derive(Subcommand)]
pub(crate) enum PerKeyAction {
    /// Set a single key to a specific color
    SetKey {
        /// Key name (e.g., A, Enter, Space, F1, etc.) - case insensitive
        key: String,
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Set multiple keys to specific colors
    SetKeys {
        /// Key-color pairs in format "key:color,key:color" (e.g., "A:red,S:green,D:blue")
        keys: String,
    },

    /// Set a range of keys to the same color using HID codes
    SetRegion {
        /// Starting HID code (0x00-0xFF)
        start_hid: u8,
        /// Number of keys to set
        count: u8,
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Turn off all per-key RGB (set all keys to black)
    Clear,

    /// Test individual key by HID code (direct addressing)
    TestMatrix {
        /// HID code (0x00-0xFF)
        hid_code: u8,
        /// Color as hex (e.g., FF0000) or name (red, green, blue, etc.)
        color: String,
    },

    /// Test a pattern across the keyboard
    TestPattern {
        /// Pattern name: rainbow, checkerboard, wave, test
        pattern: String,
    },

    /// Show keyboard mapping information
    ShowMapping,

    /// Show per-key RGB support status
    Status,
}

#[derive(Subcommand)]
pub(crate) enum ProfileAction {
    /// List all profiles
    List,

    /// Load a profile
    Load {
        /// Profile name
        name: String,
    },

    /// Save current settings as a profile
    Save {
        /// Profile name
        name: String,
    },

    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum RemapAction {
    /// Grab the keyboard and apply the `[remap]` bindings from config.toml until Ctrl-C or the escape chord
    /// (hold both Ctrl and both Shift keys for 2 seconds)
    Run,

    /// Print the configured bindings after validating them
    List,
}

#[cfg(feature = "audio")]
#[derive(Subcommand)]
pub(crate) enum AudioAction {
    /// Show current mixer state
    Status,

    /// Set channel volume
    Volume {
        /// Channel: master, game, chat, media, aux, mic
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },

    /// Mute/unmute a channel
    Mute {
        /// Channel: master, game, chat, media, aux, mic
        channel: String,

        /// Mute state (true/false), omit to toggle
        #[arg(short, long)]
        state: Option<bool>,
    },

    /// Set chat mix balance
    ChatMix {
        /// Balance (-100 = game, 0 = balanced, 100 = chat)
        balance: i8,
    },
}

#[cfg(feature = "sonar")]
#[derive(Subcommand)]
pub(crate) enum SonarAction {
    /// Show current Sonar status and volumes
    Status,

    /// Discover the Sonar API port
    Discover,

    /// Get audio devices
    Devices,

    /// Get current mode (classic or streamer)
    Mode,

    /// Set volume for a channel (classic mode)
    Volume {
        /// Channel: master, game, chat, media, aux
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },

    /// Get chat mix settings
    ChatMix,

    /// Control streamer mode
    Streamer {
        #[command(subcommand)]
        action: StreamerAction,
    },

    /// Get all configurations
    Configs,
}

#[cfg(feature = "sonar")]
#[derive(Subcommand)]
pub(crate) enum StreamerAction {
    /// Set monitoring volume for a channel
    Monitoring {
        /// Channel: master, game, chat
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },

    /// Set streaming volume for a channel
    Streaming {
        /// Channel: master, game, chat
        channel: String,

        /// Volume level (0-100)
        level: u8,
    },
}

#[derive(Subcommand)]
pub(crate) enum PollrateAction {
    /// Set mouse polling rate
    Mouse {
        /// Polling rate in Hz (125, 250, 500, 1000, 2000, 4000)
        /// Note: Rates above 1000Hz cannot be set through the Linux kernel parameter
        rate: u32,

        /// Save to config and apply on daemon startup
        #[arg(long)]
        persistent: bool,
    },

    /// Set keyboard polling rate
    Keyboard {
        /// Polling rate in Hz (125, 250, 500, 1000, 2000, 4000)
        /// Note: Rates above 1000Hz cannot be set through the Linux kernel parameter
        rate: u32,

        /// Save to config and apply on daemon startup
        #[arg(long)]
        persistent: bool,
    },

    /// Show current polling rates
    Status,
}

#[derive(Subcommand)]
pub(crate) enum PerformanceAction {
    /// Show current performance statistics
    Stats {
        /// Continuously monitor (update every N seconds)
        #[arg(short, long)]
        monitor: Option<u64>,

        /// Export stats to file
        #[arg(short, long)]
        output: Option<String>,

        /// JSON output format
        #[arg(long)]
        json: bool,
    },

    /// Enable performance optimizations
    Enable,

    /// Disable performance optimizations
    Disable,

    /// Cleanup performance caches
    Cleanup,

    /// Run performance benchmark
    Benchmark {
        /// Duration in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Export results to file
        #[arg(short, long)]
        output: Option<String>,
    },
}

pub(crate) fn parse_color(s: &str) -> Option<Color> {
    Color::parse(s)
}

pub(crate) fn require_color(s: &str) -> Result<Color> {
    parse_color(s).ok_or_else(|| Error::Other(format!("Invalid color: {}", s)))
}

pub(crate) fn parse_hex_u8(s: &str) -> std::result::Result<u8, String> {
    let s = s.trim_start_matches("0x");
    u8::from_str_radix(s, 16).map_err(|e| format!("Invalid hex value: {}", e))
}

#[cfg(feature = "audio")]
pub(crate) fn parse_channel(s: &str) -> Option<Channel> {
    let s_lower = s.to_ascii_lowercase();
    match s_lower.as_str() {
        "master" => Some(Channel::Master),
        "game" => Some(Channel::Game),
        "chat" => Some(Channel::Chat),
        "media" => Some(Channel::Media),
        "aux" => Some(Channel::Aux),
        "mic" => Some(Channel::Mic),
        _ => None,
    }
}

/// Convert a volume level (0-100) to a normalized float (0.0-1.0)
#[cfg(any(feature = "audio", feature = "sonar"))]
pub(crate) fn normalize_volume(level: u8) -> f32 {
    (level.min(100) as f32) / 100.0
}

/// Parse and validate a Sonar channel name
#[cfg(feature = "sonar")]
pub(crate) fn parse_sonar_channel<'a>(channel: &'a str, valid_channels: &[&str]) -> Result<&'a str> {
    let channel_lower = channel.to_ascii_lowercase();
    if valid_channels.contains(&channel_lower.as_str()) {
        Ok(channel)
    } else {
        Err(Error::Other(format!(
            "Invalid channel: {}. Valid channels: {}",
            channel,
            valid_channels.join(", ")
        )))
    }
}

/// Parse a zone identifier (e.g., "zone1", "2", "all") into a zone index (0-based).
/// Returns None for "all"/"keyboard" which should apply to all zones.
#[inline]
pub(crate) fn parse_zone_number(zone: &str) -> Option<usize> {
    // Fast path: check common cases without string allocation
    if zone.eq_ignore_ascii_case("all") || zone.eq_ignore_ascii_case("keyboard") {
        return None;
    }

    // Try parsing as "zone<number>" or just "<number>"
    // Use case-insensitive prefix check to avoid allocation
    let number_part = if zone.len() > 4 && zone[0..4].eq_ignore_ascii_case("zone") {
        &zone[4..]
    } else {
        zone
    };

    number_part.parse::<usize>().ok().and_then(|one_based| {
        // Convert 1-based to 0-based index
        if one_based > 0 { Some(one_based - 1) } else { None }
    })
}

pub(crate) fn parse_key_name(name: &str) -> Option<KeyId> {
    let name_upper = name.to_ascii_uppercase();
    match name_upper.as_str() {
        // Letters
        "A" => Some(KeyId::A),
        "B" => Some(KeyId::B),
        "C" => Some(KeyId::C),
        "D" => Some(KeyId::D),
        "E" => Some(KeyId::E),
        "F" => Some(KeyId::F),
        "G" => Some(KeyId::G),
        "H" => Some(KeyId::H),
        "I" => Some(KeyId::I),
        "J" => Some(KeyId::J),
        "K" => Some(KeyId::K),
        "L" => Some(KeyId::L),
        "M" => Some(KeyId::M),
        "N" => Some(KeyId::N),
        "O" => Some(KeyId::O),
        "P" => Some(KeyId::P),
        "Q" => Some(KeyId::Q),
        "R" => Some(KeyId::R),
        "S" => Some(KeyId::S),
        "T" => Some(KeyId::T),
        "U" => Some(KeyId::U),
        "V" => Some(KeyId::V),
        "W" => Some(KeyId::W),
        "X" => Some(KeyId::X),
        "Y" => Some(KeyId::Y),
        "Z" => Some(KeyId::Z),

        // Numbers
        "1" => Some(KeyId::Key1),
        "2" => Some(KeyId::Key2),
        "3" => Some(KeyId::Key3),
        "4" => Some(KeyId::Key4),
        "5" => Some(KeyId::Key5),
        "6" => Some(KeyId::Key6),
        "7" => Some(KeyId::Key7),
        "8" => Some(KeyId::Key8),
        "9" => Some(KeyId::Key9),
        "0" => Some(KeyId::Key0),

        // Function keys
        "F1" => Some(KeyId::F1),
        "F2" => Some(KeyId::F2),
        "F3" => Some(KeyId::F3),
        "F4" => Some(KeyId::F4),
        "F5" => Some(KeyId::F5),
        "F6" => Some(KeyId::F6),
        "F7" => Some(KeyId::F7),
        "F8" => Some(KeyId::F8),
        "F9" => Some(KeyId::F9),
        "F10" => Some(KeyId::F10),
        "F11" => Some(KeyId::F11),
        "F12" => Some(KeyId::F12),

        // Special keys
        "ENTER" | "RETURN" => Some(KeyId::Enter),
        "SPACE" | "SPACEBAR" => Some(KeyId::Space),
        "ESC" | "ESCAPE" => Some(KeyId::Escape),
        "TAB" => Some(KeyId::Tab),
        "SHIFT" => Some(KeyId::LeftShift),
        "CTRL" => Some(KeyId::LeftCtrl),
        "ALT" => Some(KeyId::LeftAlt),
        "WIN" | "WINDOWS" => Some(KeyId::LeftWin),
        "CAPS" | "CAPSLOCK" => Some(KeyId::CapsLock),
        "BACKSPACE" => Some(KeyId::Backspace),

        // Arrows
        "UP" | "ARROWUP" => Some(KeyId::ArrowUp),
        "DOWN" | "ARROWDOWN" => Some(KeyId::ArrowDown),
        "LEFT" | "ARROWLEFT" => Some(KeyId::ArrowLeft),
        "RIGHT" | "ARROWRIGHT" => Some(KeyId::ArrowRight),

        // Punctuation
        ";" | "SEMICOLON" => Some(KeyId::Semicolon),
        "'" | "QUOTE" => Some(KeyId::Quote),
        "," | "COMMA" => Some(KeyId::Comma),
        "." | "PERIOD" => Some(KeyId::Period),
        "/" | "SLASH" => Some(KeyId::Slash),
        "[" | "LEFTBRACKET" => Some(KeyId::LeftBracket),
        "]" | "RIGHTBRACKET" => Some(KeyId::RightBracket),
        "\\" | "BACKSLASH" => Some(KeyId::Backslash),
        "-" | "MINUS" => Some(KeyId::Minus),
        "=" | "EQUAL" => Some(KeyId::Equal),
        "`" | "BACKTICK" => Some(KeyId::Backtick),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_args() -> OnboardReadArgs {
        OnboardReadArgs {
            profile: 1,
            device: None,
            out: None,
            bin: None,
            report: None,
            hex: false,
            verify: false,
        }
    }

    fn load_args() -> OnboardLoadArgs {
        OnboardLoadArgs {
            profile: 1,
            device: None,
        }
    }

    fn decode_args() -> OnboardDecodeArgs {
        OnboardDecodeArgs {
            file: PathBuf::from("p.bin"),
            out: None,
            hex: false,
            hex_from: 0,
            hex_len: 64,
        }
    }

    fn encode_args() -> OnboardEncodeArgs {
        OnboardEncodeArgs {
            file: PathBuf::from("p.json"),
            out: None,
            check: None,
            hex: false,
            hex_from: 0,
            hex_len: 64,
        }
    }

    fn write_args(dry_run: bool) -> OnboardWriteArgs {
        OnboardWriteArgs {
            file: Some(PathBuf::from("p.json")),
            bin: None,
            profile: 1,
            device: None,
            dry_run,
            yes: false,
        }
    }

    /// `main` hands a device manager to a subcommand exactly when this says true, so the flag that stops a
    /// write from sending anything is also what keeps it from opening a keyboard.
    #[test]
    fn a_write_needs_a_keyboard_only_when_it_is_asked_to_send() {
        assert!(
            OnboardAction::Write(write_args(false)).needs_keyboard(),
            "a write without --dry-run sends frames and must enumerate the keyboard it sends them on"
        );
        assert!(
            !OnboardAction::Write(write_args(true)).needs_keyboard(),
            "a dry run prints the plan and opens no keyboard"
        );
    }

    #[test]
    fn the_on_board_subcommands_that_run_on_a_keyboard_are_read_write_and_deploy() {
        assert!(
            OnboardAction::Read(read_args()).needs_keyboard(),
            "a read pulls bytes from a keyboard's handle"
        );
        assert!(
            OnboardAction::Load(load_args()).needs_keyboard(),
            "a deploy sends a frame to a keyboard"
        );
        assert!(
            !OnboardAction::Decode(decode_args()).needs_keyboard(),
            "decode works on an image a previous read wrote"
        );
        assert!(
            !OnboardAction::Encode(encode_args()).needs_keyboard(),
            "encode writes bytes and sends nothing"
        );
    }

    #[test]
    fn mouse_poll_rate_is_a_separate_command_from_the_os_pollrate() {
        let parsed = Cli::try_parse_from(["ssgg", "mouse", "poll-rate", "1000"]).unwrap();
        assert!(matches!(
            parsed.command,
            Commands::Mouse {
                action: MouseAction::PollRate { hz: 1000 }
            }
        ));
        let parsed = Cli::try_parse_from(["ssgg", "mouse", "dpi", "800"]).unwrap();
        assert!(matches!(
            parsed.command,
            Commands::Mouse {
                action: MouseAction::Dpi { value: 800 }
            }
        ));
        assert!(Cli::try_parse_from(["ssgg", "mouse", "save"]).is_err());
        assert!(Cli::try_parse_from(["ssgg", "pollrate", "mouse", "1000"]).is_ok());
    }

    /// clap only checks its own definition (duplicate flags, bad defaults, conflicting shorts) when a
    /// command is built; without this a bad definition surfaces as a panic on the user's first run.
    #[test]
    fn the_clap_definition_is_internally_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}

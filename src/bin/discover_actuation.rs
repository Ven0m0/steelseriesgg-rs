use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use hidapi::{HidApi, HidDevice};
use std::thread;
use std::time::Duration;

/// SteelSeries Actuation Point Discovery Tool
///
/// This tool attempts to discover the "Read Actuation Point" HID command
/// by fuzzing the device with commands and correlating responses with
/// known actuation values set via the known "Write Actuation Point" command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Vendor ID (default: SteelSeries 0x1038)
    #[arg(long, default_value = "0x1038", value_parser = parse_hex)]
    vid: u16,

    /// Product ID of the keyboard (e.g., Apex Pro TKL)
    /// If not provided, the tool will list available SteelSeries devices.
    #[arg(long, value_parser = parse_hex)]
    pid: Option<u16>,

    /// Interface number to connect to (usually 1 for main control)
    #[arg(long, default_value = "1")]
    interface: i32,

    /// Force execution (required to run the fuzzer)
    #[arg(long)]
    force: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn parse_hex(s: &str) -> Result<u16, std::num::ParseIntError> {
    let s = s.trim_start_matches("0x");
    u16::from_str_radix(s, 16)
}

const REPORT_SIZE: usize = 64;
const CMD_SET_ACTUATION: u8 = 0x2D;

// Known dangerous or irrelevant commands to skip
const SKIP_COMMANDS: &[u8] = &[
    0x00,              // Invalid
    CMD_SET_ACTUATION, // The write command itself
    0x30,              // RGB Data
    0x20,              // Other known writes (safety)
    0x22,              // Other known writes (safety)
    0x2A,              // RGB Per Key
    0x02,              // Firmware related?
    0x90,              // Firmware update start (very dangerous)
    0x91,              // Firmware update
    0x92,              // Firmware update
];

fn main() -> Result<()> {
    let args = Args::parse();

    println!("{}", "SteelSeries Actuation Discovery Tool".bold().cyan());
    println!("========================================");

    if !args.force {
        println!(
            "{}",
            "WARNING: This tool sends random HID commands to your device.".yellow()
        );
        println!("While generally safe for queries, there is a non-zero risk of unintended side effects.");
        println!("Please run with --force to confirm you understand the risks.");
        return Ok(());
    }

    let api = HidApi::new().context("Failed to initialize HID API")?;

    // If no PID provided, list devices
    let Some(pid) = args.pid else {
        println!(
            "No Product ID provided. Scanning for SteelSeries devices (VID: 0x{:04X})...",
            args.vid
        );
        let devices = api.device_list();
        let mut found = false;
        for dev in devices {
            if dev.vendor_id() == args.vid {
                println!(
                    "Found: {} [PID: 0x{:04X}] (Interface: {})",
                    dev.product_string().unwrap_or("Unknown"),
                    dev.product_id(),
                    dev.interface_number()
                );
                found = true;
            }
        }
        if !found {
            println!("No SteelSeries devices found.");
        } else {
            println!("\nPlease rerun with --pid <PID> (and --interface <NUM> if needed).");
            println!("Example: cargo run --bin discover_actuation -- --pid 0x161C --force");
        }
        return Ok(());
    };

    println!(
        "Opening device VID: 0x{:04X} PID: 0x{:04X} Interface: {}...",
        args.vid, pid, args.interface
    );

    // Find device path for specific interface
    let device_info = api
        .device_list()
        .into_iter()
        .find(|d| d.vendor_id() == args.vid && d.product_id() == pid && d.interface_number() == args.interface)
        .context("Device not found with specified VID/PID/Interface")?;

    let device = api.open_path(device_info.path()).context("Failed to open device")?;
    println!("{}", "Device opened successfully.".green());

    run_discovery(&device, args.verbose)?;

    Ok(())
}

fn run_discovery(device: &HidDevice, verbose: bool) -> Result<()> {
    println!("\nStarting Discovery Process...");
    println!("-----------------------------");

    // Phase 1: Set Baseline
    let baseline_val = 20; // 2.0mm
    println!("Phase 1: Setting baseline actuation to {} (2.0mm)...", baseline_val);
    set_actuation(device, baseline_val)?;
    thread::sleep(Duration::from_millis(500)); // Wait for settle

    // Phase 2: Scan
    println!("Phase 2: Scanning command codes (0x00 - 0xFF)...");
    let mut candidates = Vec::new();

    for cmd_code in 0x00..=0xFFu8 {
        if SKIP_COMMANDS.contains(&cmd_code) {
            continue;
        }

        if verbose {
            print!("\rTesting 0x{:02X}...", cmd_code);
        }

        match send_command(device, cmd_code) {
            Ok(response) => {
                // Check if response contains our baseline value
                if response.contains(&baseline_val) {
                    println!(
                        "\n{} Response to 0x{:02X} contains baseline value ({})",
                        "MATCH:".green(),
                        cmd_code,
                        baseline_val
                    );
                    print_buffer(&response);
                    candidates.push(cmd_code);
                }
            }
            Err(_) => {
                // Timeout or error, ignore
            }
        }
        thread::sleep(Duration::from_millis(10)); // Slight delay
    }

    if candidates.is_empty() {
        println!("\n{}", "No candidates found during scan.".red());
        return Ok(());
    }

    println!("\nFound {} candidates: {:02X?}", candidates.len(), candidates);

    // Phase 3: Verify
    let verify_val = 35; // 3.5mm
    println!(
        "\nPhase 3: Verifying candidates with new value {} (3.5mm)...",
        verify_val
    );
    set_actuation(device, verify_val)?;
    thread::sleep(Duration::from_millis(500));

    let mut confirmed = Vec::new();

    for &cmd in &candidates {
        println!("Verifying 0x{:02X}...", cmd);
        match send_command(device, cmd) {
            Ok(response) => {
                if response.contains(&verify_val) {
                    println!(
                        "{}",
                        "CONFIRMED! This command reflects the new actuation value."
                            .bold()
                            .green()
                    );
                    print_buffer(&response);

                    // Find index of value
                    if let Some(idx) = response.iter().position(|&x| x == verify_val) {
                        println!("Value found at index: {}", idx);
                    }

                    confirmed.push(cmd);
                } else {
                    println!(
                        "Candidate 0x{:02X} did not update (still contains old val or random data).",
                        cmd
                    );
                }
            }
            Err(e) => println!("Failed to send verify command: {}", e),
        }
        thread::sleep(Duration::from_millis(100));
    }

    println!("\n========================================");
    if !confirmed.is_empty() {
        println!("{}", "DISCOVERY SUCCESSFUL".bold().green());
        println!("The following commands read actuation data:");
        for cmd in confirmed {
            println!("  - 0x{:02X}", cmd);
        }
    } else {
        println!("{}", "Discovery failed. No consistent read command found.".yellow());
    }

    Ok(())
}

fn set_actuation(device: &HidDevice, val: u8) -> Result<()> {
    let mut data = [0u8; REPORT_SIZE + 1];
    data[0] = 0x00; // Report ID
    data[1] = CMD_SET_ACTUATION;
    data[2] = val;

    device.write(&data).context("Failed to write actuation command")?;
    Ok(())
}

fn send_command(device: &HidDevice, cmd: u8) -> Result<Vec<u8>> {
    let mut data = [0u8; REPORT_SIZE + 1];
    data[0] = 0x00; // Report ID
    data[1] = cmd;

    // Send
    device.write(&data).context("Write failed")?;

    // Read response with timeout
    let mut buf = [0u8; REPORT_SIZE + 1];
    let res = device.read_timeout(&mut buf, 100).context("Read failed")?; // 100ms timeout

    if res > 0 {
        Ok(buf[..res].to_vec())
    } else {
        Err(anyhow::anyhow!("No response"))
    }
}

fn print_buffer(buf: &[u8]) {
    print!("   [");
    for (i, b) in buf.iter().enumerate() {
        // Skip Report ID if 0 at start usually, but let's print all
        if i > 0 && i % 16 == 0 {
            print!("\n    ");
        }
        print!("{:02X} ", b);
    }
    println!("]");
}

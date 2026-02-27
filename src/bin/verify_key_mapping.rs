use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use hidapi::{HidApi, HidDevice};
use std::ffi::CString;
use std::io::{self, Write};

/// SteelSeries Key Mapping Verification Tool
///
/// This tool helps discover the correct HID matrix addresses for keys on
/// SteelSeries keyboards by systematically lighting up keys and asking the user
/// to identify them.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Product ID to target (default: Apex Pro TKL 2023 - 0x1628)
    #[arg(short, long, default_value_t = 0x1628)]
    product_id: u16,

    /// Start Row (0-255)
    #[arg(long, default_value_t = 0)]
    start_row: u8,

    /// End Row (0-255)
    #[arg(long, default_value_t = 7)]
    end_row: u8,

    /// Start Column (0-255)
    #[arg(long, default_value_t = 0)]
    start_col: u8,

    /// End Column (0-255)
    #[arg(long, default_value_t = 20)]
    end_col: u8,

    /// Manual mode: test a specific address
    #[arg(long)]
    manual: bool,

    /// Fuzz mode: test command formats instead of matrix scanning
    #[arg(long)]
    fuzz: bool,

    /// Command byte to use for per-key control (default: 0xB0)
    #[arg(long, default_value_t = 0xB0)]
    command_byte: u8,
}

const VENDOR_ID: u16 = 0x1038;
const CONTROL_INTERFACE: i32 = 1;

fn connect_device(api: &HidApi, product_id: u16) -> Result<HidDevice> {
    println!("Scanning for device {:04x}:{:04x}...", VENDOR_ID, product_id);

    // List devices to find the correct interface
    let mut target_path: Option<CString> = None;

    for device in api.device_list() {
        if device.vendor_id() == VENDOR_ID && device.product_id() == product_id {
            if device.interface_number() == CONTROL_INTERFACE || device.interface_number() == -1 {
                target_path = Some(CString::from(device.path()));
                break;
            }
        }
    }

    if let Some(path) = target_path {
        println!("Found device at path: {:?}", path);
        let device = api.open_path(&path).context("Failed to open HID device")?;
        Ok(device)
    } else {
        println!("Specific interface not found, trying generic open...");
        api.open(VENDOR_ID, product_id)
            .context(format!("Device {:04x}:{:04x} not found", VENDOR_ID, product_id))
    }
}

fn send_packet(device: &HidDevice, packet: &[u8]) -> Result<()> {
    let mut report = vec![0u8; 65];
    report[0] = 0x00;

    if packet.len() > 64 {
        return Err(anyhow::anyhow!("Packet too long (max 64 bytes)"));
    }

    report[1..1 + packet.len()].copy_from_slice(packet);

    device.write(&report).context("Failed to write HID report")?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("{}", "SteelSeries Key Mapping Verification Tool".bold().green());
    println!("Target Product ID: {:#06x}", args.product_id);

    let api = HidApi::new().context("Failed to initialize HID API")?;

    match connect_device(&api, args.product_id) {
        Ok(device) => {
            println!("{}", "Device connected successfully!".green());

            if args.fuzz {
                fuzz_mode(&device)?;
            } else if args.manual {
                manual_mode(&device, &args)?;
            } else {
                scan_mode(&device, &args)?;
            }
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            eprintln!("Make sure udev rules are installed and you have permissions.");
            return Err(e);
        }
    }

    Ok(())
}

fn fuzz_mode(device: &HidDevice) -> Result<()> {
    println!("{}", "ENTERING FUZZ MODE".bold().yellow());
    println!("This mode will iterate through potential command bytes to find per-key RGB control.");
    println!("We will target Row 0, Col 0 (usually ESC) with RED color.");

    let candidates = vec![
        0x21, // Known Zone command
        0xB0, // Common per-key
        0x20, 0x22, 0x23, 0x2A, 0x2B, // Nearby values
        0x0E, 0x0D, // Older protocols
    ];

    for &cmd in &candidates {
        // Try Pattern A: [CMD, ROW, COL, R, G, B]
        let packet_a = vec![cmd, 0x00, 0x00, 0xFF, 0x00, 0x00];
        println!("\nTesting Command: 0x{:02x} (Pattern: [CMD, 00, 00, FF, 00, 00])", cmd);

        if let Err(e) = send_packet(device, &packet_a) {
            println!("Failed to send: {}", e);
            continue;
        }

        print!("Did ESC key turn RED? (y/n/q): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input == "y" {
            println!(
                "{}",
                format!("FOUND CANDIDATE: 0x{:02x} (Pattern A)", cmd).green().bold()
            );
            return Ok(());
        } else if input == "q" {
            return Ok(());
        }

        // Try Pattern B: [CMD, 0x00, ROW, COL, R, G, B]
        let packet_b = vec![cmd, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00];

        println!(
            "Testing Command: 0x{:02x} (Pattern: [CMD, 00, 00, 00, FF, 00, 00])",
            cmd
        );
        if let Err(e) = send_packet(device, &packet_b) {
            println!("Failed to send: {}", e);
            continue;
        }

        print!("Did ESC key turn RED? (y/n/q): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input == "y" {
            println!(
                "{}",
                format!("FOUND CANDIDATE: 0x{:02x} (Pattern B)", cmd).green().bold()
            );
            return Ok(());
        } else if input == "q" {
            return Ok(());
        }
    }

    println!("Fuzzing complete. No candidate found in preset list.");
    Ok(())
}

fn manual_mode(device: &HidDevice, args: &Args) -> Result<()> {
    println!("{}", "ENTERING MANUAL MODE".bold().yellow());
    println!("Testing Address: Row {}, Col {}", args.start_row, args.start_col);
    println!("Using Command Byte: 0x{:02x}", args.command_byte);

    let packet = vec![args.command_byte, args.start_row, args.start_col, 0xFF, 0x00, 0x00];
    if let Err(e) = send_packet(device, &packet) {
        eprintln!("Failed to send packet: {}", e);
        return Err(e);
    }

    println!(
        "Packet sent. Did the key at ({}, {}) turn RED?",
        args.start_row, args.start_col
    );
    Ok(())
}

fn scan_mode(device: &HidDevice, args: &Args) -> Result<()> {
    println!("{}", "ENTERING SCAN MODE".bold().yellow());
    println!(
        "Scanning Matrix from ({}, {}) to ({}, {})",
        args.start_row, args.start_col, args.end_row, args.end_col
    );
    println!("Using Command Byte: 0x{:02x}", args.command_byte);
    println!("Press 'Enter' to confirm key name, 'n' for no light, 'q' to quit, 's' to skip row.");

    let mut mappings: Vec<(u8, u8, String)> = Vec::new();

    for row in args.start_row..=args.end_row {
        println!("{}", format!("\nScanning Row {}", row).bold().blue());

        for col in args.start_col..=args.end_col {
            // Send RED to current key
            let packet = vec![args.command_byte, row, col, 0xFF, 0x00, 0x00];
            if let Err(e) = send_packet(device, &packet) {
                eprintln!("Failed to send packet: {}", e);
                continue;
            }

            print!("Key at ({}, {}): ", row, col);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input == "q" {
                println!("Aborting scan.");
                print_results(&mappings);
                return Ok(());
            } else if input == "s" {
                println!("Skipping rest of row {}", row);
                break;
            } else if input == "n" || input.is_empty() {
                // No key or skipped
                continue;
            } else {
                // Key identified
                mappings.push((row, col, input.to_string()));

                // Turn it off (black) to verify/clear
                let clear_packet = vec![args.command_byte, row, col, 0x00, 0x00, 0x00];
                let _ = send_packet(device, &clear_packet);
            }
        }
    }

    println!("\nScan Complete!");
    print_results(&mappings);
    Ok(())
}

fn print_results(mappings: &[(u8, u8, String)]) {
    println!("\n{}", "Generated Mapping Code:".bold().green());
    println!("------------------------------------------------");
    for (row, col, name) in mappings {
        println!("mapping.add_key(KeyId::{}, KeyAddress::new({}, {}));", name, row, col);
    }
    println!("------------------------------------------------");
}

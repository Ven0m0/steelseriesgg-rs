// Integration test to verify device readback capabilities
// This helps determine if the device acknowledges RGB commands

use steelseries_gg::devices::{DeviceManager, DeviceType};
use steelseries_gg::rgb::Color;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Testing device readback capabilities...\n");

    let manager = DeviceManager::new()?;
    let keyboard_info = manager
        .first_device_of_type(DeviceType::Keyboard)
        .ok_or_else(|| anyhow::anyhow!("No keyboard found"))?;

    println!("Device: {}", keyboard_info.name);
    println!("Path: {}", keyboard_info.path);
    println!("Interface: {}", keyboard_info.interface_number);
    println!();

    // Open the device
    let mut keyboard = manager.open_keyboard(keyboard_info)?;

    // Try to send a color command
    println!("Sending RGB red command...");
    keyboard.set_color(Color::RED).await?;
    println!("Command sent successfully");

    // Try to read back any response (if device supports it)
    println!("\nAttempting to read device response...");
    let mut buf = [0u8; 65];

    match keyboard.receive_raw(&mut buf) {
        Ok(len) => {
            println!("Received {} bytes: {:02x?}", len, &buf[..len]);
        }
        Err(e) => {
            println!("No response from device (this is normal): {}", e);
        }
    }

    println!("\nTest complete!");
    Ok(())
}

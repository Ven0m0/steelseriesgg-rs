//! USB polling rate control for mice and keyboards.
//!
//! This module allows changing the USB HID polling rate via sysfs kernel parameters.
//! Requires root/sudo privileges to modify system files.

use crate::{Error, Result};

/// USB polling rate options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollRate {
    /// 125 Hz (8ms interval)
    Hz125,
    /// 500 Hz (2ms interval)
    Hz500,
    /// 1000 Hz (1ms interval)
    Hz1000,
    /// 2000 Hz (0.5ms interval) - Requires hardware support
    Hz2000,
    /// 4000 Hz (0.25ms interval) - Requires hardware support
    Hz4000,
}

impl PollRate {
    /// Convert poll rate to sysfs parameter value.
    pub fn to_sysfs_value(&self) -> u8 {
        match self {
            PollRate::Hz125 => 0,
            PollRate::Hz500 => 1,
            PollRate::Hz1000 => 2,
            PollRate::Hz2000 => 3,
            PollRate::Hz4000 => 4,
        }
    }

    /// Convert from sysfs parameter value to poll rate.
    pub fn from_sysfs_value(value: u8) -> Result<Self> {
        match value {
            0 => Ok(PollRate::Hz125),
            1 => Ok(PollRate::Hz500),
            2 => Ok(PollRate::Hz1000),
            3 => Ok(PollRate::Hz2000),
            4 => Ok(PollRate::Hz4000),
            _ => Err(Error::InvalidConfig(format!(
                "Invalid poll rate value: {} (valid range: 0-4)",
                value
            ))),
        }
    }

    /// Convert from frequency in Hz to poll rate enum.
    pub fn from_hz(hz: u32) -> Result<Self> {
        match hz {
            125 => Ok(PollRate::Hz125),
            500 => Ok(PollRate::Hz500),
            1000 => Ok(PollRate::Hz1000),
            2000 => Ok(PollRate::Hz2000),
            4000 => Ok(PollRate::Hz4000),
            _ => Err(Error::InvalidConfig(format!(
                "Unsupported poll rate: {} Hz (supported: 125, 500, 1000, 2000, 4000)",
                hz
            ))),
        }
    }

    /// Convert poll rate to frequency in Hz.
    pub fn to_hz(&self) -> u32 {
        match self {
            PollRate::Hz125 => 125,
            PollRate::Hz500 => 500,
            PollRate::Hz1000 => 1000,
            PollRate::Hz2000 => 2000,
            PollRate::Hz4000 => 4000,
        }
    }

    /// Check if this poll rate requires special hardware support.
    pub fn requires_hardware_support(&self) -> bool {
        matches!(self, PollRate::Hz2000 | PollRate::Hz4000)
    }

    /// Get a human-readable description of the poll rate.
    pub fn description(&self) -> String {
        match self {
            PollRate::Hz125 => "125 Hz (8ms) - Power saving".to_string(),
            PollRate::Hz500 => "500 Hz (2ms) - Standard".to_string(),
            PollRate::Hz1000 => "1000 Hz (1ms) - Gaming".to_string(),
            PollRate::Hz2000 => "2000 Hz (0.5ms) - High-end gaming (requires hardware support)".to_string(),
            PollRate::Hz4000 => "4000 Hz (0.25ms) - Enthusiast (requires hardware support)".to_string(),
        }
    }
}

/// Device type for poll rate control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Mouse device
    Mouse,
    /// Keyboard device
    Keyboard,
}

impl DeviceType {
    /// Get the sysfs path for this device type.
    fn sysfs_path(&self) -> &'static str {
        match self {
            DeviceType::Mouse => "/sys/module/usbhid/parameters/mousepoll",
            DeviceType::Keyboard => "/sys/module/usbhid/parameters/kbdpoll",
        }
    }

    /// Get the display name for this device type.
    pub fn name(&self) -> &'static str {
        match self {
            DeviceType::Mouse => "mouse",
            DeviceType::Keyboard => "keyboard",
        }
    }
}

/// Check if the current process is running as root.
fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Set USB polling rate for a device type.
///
/// # Arguments
/// * `device_type` - The type of device (mouse or keyboard)
/// * `rate` - The desired polling rate
///
/// # Errors
/// Returns an error if:
/// - The process doesn't have root privileges
/// - The sysfs file doesn't exist or can't be written
///
/// # Example
/// ```no_run
/// use steelseries_gg::pollrate::{set_poll_rate, DeviceType, PollRate};
///
/// // Requires root privileges
/// set_poll_rate(DeviceType::Mouse, PollRate::Hz1000)?;
/// # Ok::<(), steelseries_gg::Error>(())
/// ```
pub fn set_poll_rate(device_type: DeviceType, rate: PollRate) -> Result<()> {
    if !is_root() {
        return Err(Error::PermissionDenied(format!(
            "Poll rate changes require root privileges. Try: sudo ssgg pollrate {} {}",
            device_type.name(),
            rate.to_hz()
        )));
    }

    let path = device_type.sysfs_path();
    let value = rate.to_sysfs_value().to_string();

    std::fs::write(path, &value).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to write poll rate to {}: {}. Is the usbhid module loaded?",
                path, e
            ),
        ))
    })?;

    Ok(())
}

/// Get the current USB polling rate for a device type.
///
/// # Arguments
/// * `device_type` - The type of device (mouse or keyboard)
///
/// # Errors
/// Returns an error if:
/// - The sysfs file doesn't exist or can't be read
/// - The value in the file is invalid
///
/// # Example
/// ```no_run
/// use steelseries_gg::pollrate::{get_poll_rate, DeviceType};
///
/// let rate = get_poll_rate(DeviceType::Mouse)?;
/// println!("Current mouse poll rate: {} Hz", rate.to_hz());
/// # Ok::<(), steelseries_gg::Error>(())
/// ```
pub fn get_poll_rate(device_type: DeviceType) -> Result<PollRate> {
    let path = device_type.sysfs_path();

    let content = std::fs::read_to_string(path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to read poll rate from {}: {}. Is the usbhid module loaded?",
                path, e
            ),
        ))
    })?;

    let value: u8 = content
        .trim()
        .parse()
        .map_err(|e| Error::InvalidConfig(format!("Invalid poll rate value in {}: {}", path, e)))?;

    PollRate::from_sysfs_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poll_rate_conversion() {
        assert_eq!(PollRate::Hz125.to_sysfs_value(), 0);
        assert_eq!(PollRate::Hz500.to_sysfs_value(), 1);
        assert_eq!(PollRate::Hz1000.to_sysfs_value(), 2);
        assert_eq!(PollRate::Hz2000.to_sysfs_value(), 3);
        assert_eq!(PollRate::Hz4000.to_sysfs_value(), 4);

        assert_eq!(PollRate::from_sysfs_value(0).unwrap(), PollRate::Hz125);
        assert_eq!(PollRate::from_sysfs_value(1).unwrap(), PollRate::Hz500);
        assert_eq!(PollRate::from_sysfs_value(2).unwrap(), PollRate::Hz1000);
        assert_eq!(PollRate::from_sysfs_value(3).unwrap(), PollRate::Hz2000);
        assert_eq!(PollRate::from_sysfs_value(4).unwrap(), PollRate::Hz4000);
        assert!(PollRate::from_sysfs_value(5).is_err());
    }

    #[test]
    fn test_hz_conversion() {
        assert_eq!(PollRate::from_hz(125).unwrap(), PollRate::Hz125);
        assert_eq!(PollRate::from_hz(500).unwrap(), PollRate::Hz500);
        assert_eq!(PollRate::from_hz(1000).unwrap(), PollRate::Hz1000);
        assert_eq!(PollRate::from_hz(2000).unwrap(), PollRate::Hz2000);
        assert_eq!(PollRate::from_hz(4000).unwrap(), PollRate::Hz4000);
        assert!(PollRate::from_hz(8000).is_err());
        assert!(PollRate::from_hz(250).is_err());

        assert_eq!(PollRate::Hz125.to_hz(), 125);
        assert_eq!(PollRate::Hz500.to_hz(), 500);
        assert_eq!(PollRate::Hz1000.to_hz(), 1000);
        assert_eq!(PollRate::Hz2000.to_hz(), 2000);
        assert_eq!(PollRate::Hz4000.to_hz(), 4000);
    }

    #[test]
    fn test_hardware_support_check() {
        assert!(!PollRate::Hz125.requires_hardware_support());
        assert!(!PollRate::Hz500.requires_hardware_support());
        assert!(!PollRate::Hz1000.requires_hardware_support());
        assert!(PollRate::Hz2000.requires_hardware_support());
        assert!(PollRate::Hz4000.requires_hardware_support());
    }

    #[test]
    fn test_descriptions() {
        assert!(PollRate::Hz125.description().contains("125 Hz"));
        assert!(PollRate::Hz4000.description().contains("hardware support"));
    }

    #[test]
    fn test_device_type_paths() {
        assert_eq!(
            DeviceType::Mouse.sysfs_path(),
            "/sys/module/usbhid/parameters/mousepoll"
        );
        assert_eq!(
            DeviceType::Keyboard.sysfs_path(),
            "/sys/module/usbhid/parameters/kbdpoll"
        );
    }
}

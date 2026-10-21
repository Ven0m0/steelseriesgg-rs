//! USB polling rate control for mice and keyboards.
//!
//! This module allows changing the USB HID polling rate via sysfs kernel parameters.
//! Requires root/sudo privileges to modify system files.
//!
//! The `usbhid` parameters (`mousepoll`, `kbpoll`) hold an interval in whole **milliseconds**
//! (`0` = use the device's own interval), so the kernel route tops out at 1000 Hz.

use crate::{Error, Result};
#[cfg(target_os = "linux")]
use rustix::process::geteuid;

/// USB polling rate options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollRate {
    /// 125 Hz (8ms interval)
    Hz125,
    /// 250 Hz (4ms interval)
    Hz250,
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
    /// Convert poll rate to the `usbhid` parameter value (an interval in milliseconds).
    ///
    /// Rates above 1000 Hz need a sub-millisecond interval the kernel parameter cannot express.
    pub fn to_sysfs_value(&self) -> Result<u8> {
        match self {
            PollRate::Hz125 => Ok(8),
            PollRate::Hz250 => Ok(4),
            PollRate::Hz500 => Ok(2),
            PollRate::Hz1000 => Ok(1),
            PollRate::Hz2000 | PollRate::Hz4000 => Err(Error::InvalidConfig(format!(
                "{} Hz cannot be set through usbhid: its interval is in whole milliseconds, so 1000 Hz is the maximum",
                self.to_hz()
            ))),
        }
    }

    /// Convert a `usbhid` parameter value (interval in milliseconds) to a poll rate.
    pub fn from_sysfs_value(value: u8) -> Result<Self> {
        match value {
            8 => Ok(PollRate::Hz125),
            4 => Ok(PollRate::Hz250),
            2 => Ok(PollRate::Hz500),
            1 => Ok(PollRate::Hz1000),
            _ => Err(Error::InvalidConfig(format!(
                "Unsupported usbhid poll interval: {} ms (supported: 1, 2, 4, 8)",
                value
            ))),
        }
    }

    /// Convert from frequency in Hz to poll rate enum.
    pub fn from_hz(hz: u32) -> Result<Self> {
        match hz {
            125 => Ok(PollRate::Hz125),
            250 => Ok(PollRate::Hz250),
            500 => Ok(PollRate::Hz500),
            1000 => Ok(PollRate::Hz1000),
            2000 => Ok(PollRate::Hz2000),
            4000 => Ok(PollRate::Hz4000),
            _ => Err(Error::InvalidConfig(format!(
                "Unsupported poll rate: {} Hz (supported: 125, 250, 500, 1000, 2000, 4000)",
                hz
            ))),
        }
    }

    /// Convert poll rate to frequency in Hz.
    pub fn to_hz(&self) -> u32 {
        match self {
            PollRate::Hz125 => 125,
            PollRate::Hz250 => 250,
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
            PollRate::Hz250 => "250 Hz (4ms) - Office".to_string(),
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
    #[cfg(target_os = "linux")]
    fn sysfs_path(&self) -> &'static str {
        match self {
            DeviceType::Mouse => "/sys/module/usbhid/parameters/mousepoll",
            DeviceType::Keyboard => "/sys/module/usbhid/parameters/kbpoll",
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

/// Check if the current process is running as root (Linux only).
#[cfg(target_os = "linux")]
fn is_root() -> bool {
    geteuid().as_raw() == 0
}

/// Set USB polling rate for a device type.
///
/// On Linux this writes the kernel usbhid module parameter via sysfs and requires root.
/// On other platforms this operation is not supported.
#[cfg(target_os = "linux")]
pub async fn set_poll_rate(device_type: DeviceType, rate: PollRate) -> Result<()> {
    // Validate first so an unsettable rate fails before asking for root.
    let value = rate.to_sysfs_value()?.to_string();

    if !is_root() {
        let bin_name = match std::env::current_exe() {
            Ok(path) => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "ssgg".to_string()),
            Err(_) => "ssgg".to_string(),
        };

        return Err(Error::PermissionDenied(format!(
            "Poll rate changes require root privileges. Try: sudo {} pollrate {} {}",
            bin_name,
            device_type.name(),
            rate.to_hz()
        )));
    }

    let path = device_type.sysfs_path();

    tokio::fs::write(path, &value).await.map_err(|e| {
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

#[cfg(not(target_os = "linux"))]
#[allow(clippy::needless_return)]
pub async fn set_poll_rate(device_type: DeviceType, rate: PollRate) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let ms = 1000 / rate.to_hz();
        return tokio::task::spawn_blocking(move || {
            windows_hid_poll_ioctl(device_type, Some(ms))?;
            Ok(())
        })
        .await
        .map_err(|e| Error::Other(format!("Blocking task error: {e}")))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device_type, rate);
        Err(Error::Other(
            "Poll rate control is not supported on this platform".to_string(),
        ))
    }
}

/// Get the current USB polling rate for a device type.
///
/// On Linux this reads the kernel usbhid module parameter via sysfs.
/// A sysfs value of 0 means no kernel override is active; the device runs at its
/// hardware default interval (typically 1000 Hz for SteelSeries keyboards).
/// On Windows this queries the HID class driver via IOCTL_HID_GET_POLL_FREQUENCY_MSEC.
#[cfg(target_os = "linux")]
pub async fn get_poll_rate(device_type: DeviceType) -> Result<PollRate> {
    let path = device_type.sysfs_path();

    let content = tokio::fs::read_to_string(path).await.map_err(|e| {
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

    // sysfs value 0 means the kernel has no override and the device uses its
    // hardware bInterval. SteelSeries keyboards default to 1000 Hz.
    if value == 0 {
        return Ok(PollRate::Hz1000);
    }

    PollRate::from_sysfs_value(value)
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::needless_return)]
pub async fn get_poll_rate(device_type: DeviceType) -> Result<PollRate> {
    #[cfg(target_os = "windows")]
    {
        return tokio::task::spawn_blocking(move || {
            let ms = windows_hid_poll_ioctl(device_type, None)?;
            // 0 ms means the driver is using the device's native hardware interval
            // (typically 1 ms = 1000 Hz for SteelSeries keyboards).
            if ms == 0 {
                return Ok(PollRate::Hz1000);
            }
            let hz = 1000u32 / ms;
            PollRate::from_hz(hz)
        })
        .await
        .map_err(|e| Error::Other(format!("Blocking task error: {e}")))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = device_type;
        Err(Error::Other(
            "Poll rate detection is not supported on this platform".to_string(),
        ))
    }
}

/// Send a Windows HID poll-frequency IOCTL.
///
/// `set_ms = None`      → GET; returns the current interval in milliseconds.
/// `set_ms = Some(ms)`  → SET; programs the interval and returns ms on success.
///
/// Uses `IOCTL_HID_GET_POLL_FREQUENCY_MSEC` / `IOCTL_HID_SET_POLL_FREQUENCY_MSEC`
/// (hidclass.sys, FILE_ANY_ACCESS — no elevated privileges required for GET;
///  SET may be restricted depending on the Windows version and driver).
#[cfg(target_os = "windows")]
fn windows_hid_poll_ioctl(device_type: DeviceType, set_ms: Option<u32>) -> Result<u32> {
    use hidapi::HidApi;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    // HID_CTL_CODE(n) = CTL_CODE(FILE_DEVICE_KEYBOARD=0x0B, n, METHOD_BUFFERED=0, FILE_ANY_ACCESS=0)
    const IOCTL_HID_GET_POLL_FREQUENCY_MSEC: u32 = 0x000B_0024; // HID_CTL_CODE(9)
    const IOCTL_HID_SET_POLL_FREQUENCY_MSEC: u32 = 0x000B_0028; // HID_CTL_CODE(10)

    let (usage_page, usage): (u16, u16) = match device_type {
        DeviceType::Keyboard => (0x0001, 0x0006), // Generic Desktop / Keyboard
        DeviceType::Mouse => (0x0001, 0x0002),    // Generic Desktop / Mouse
    };

    let api = HidApi::new().map_err(|e| Error::DeviceCommunication(format!("HID API init failed: {e}")))?;

    let path = api
        .device_list()
        .find(|d| d.vendor_id() == crate::STEELSERIES_VENDOR_ID && d.usage_page() == usage_page && d.usage() == usage)
        .ok_or_else(|| {
            Error::DeviceNotFound(format!(
                "No SteelSeries {} found for poll-rate query",
                device_type.name()
            ))
        })?
        .path()
        .to_string_lossy()
        .into_owned();

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

    // FILE_ANY_ACCESS (dwDesiredAccess = 0) is sufficient for both GET and SET IOCTLs.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(Error::DeviceCommunication(format!(
            "Failed to open HID device '{}': {}",
            path,
            std::io::Error::last_os_error()
        )));
    }

    let result: Result<u32> = if let Some(ms) = set_ms {
        let mut bytes_ret: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_HID_SET_POLL_FREQUENCY_MSEC,
                (&ms as *const u32).cast(),
                std::mem::size_of::<u32>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_ret,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(Error::DeviceCommunication(format!(
                "IOCTL_HID_SET_POLL_FREQUENCY_MSEC failed: {}",
                std::io::Error::last_os_error()
            )))
        } else {
            Ok(ms)
        }
    } else {
        let mut poll_ms: u32 = 0;
        let mut bytes_ret: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_HID_GET_POLL_FREQUENCY_MSEC,
                std::ptr::null(),
                0,
                (&mut poll_ms as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
                &mut bytes_ret,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let os_err = std::io::Error::last_os_error();
            // ERROR_INVALID_FUNCTION (1): driver doesn't implement this IOCTL.
            // hidusb.sys (USB HID) never supports it; only PS/2 kbdhid does.
            if os_err.raw_os_error() == Some(1) {
                Err(Error::DeviceCommunication(
                    "Poll rate query is not supported by this device's HID driver. \
                     USB HID devices (including SteelSeries keyboards) do not expose \
                     poll rate via the Windows HID class IOCTL interface."
                        .into(),
                ))
            } else {
                Err(Error::DeviceCommunication(format!(
                    "IOCTL_HID_GET_POLL_FREQUENCY_MSEC failed: {os_err}"
                )))
            }
        } else {
            Ok(poll_ms)
        }
    };

    unsafe { CloseHandle(handle) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poll_rate_conversion() {
        // usbhid takes an interval in milliseconds: Hz = 1000 / value.
        assert_eq!(PollRate::Hz125.to_sysfs_value().unwrap(), 8);
        assert_eq!(PollRate::Hz250.to_sysfs_value().unwrap(), 4);
        assert_eq!(PollRate::Hz500.to_sysfs_value().unwrap(), 2);
        assert_eq!(PollRate::Hz1000.to_sysfs_value().unwrap(), 1);
        assert!(PollRate::Hz2000.to_sysfs_value().is_err());
        assert!(PollRate::Hz4000.to_sysfs_value().is_err());

        assert_eq!(PollRate::from_sysfs_value(8).unwrap(), PollRate::Hz125);
        assert_eq!(PollRate::from_sysfs_value(4).unwrap(), PollRate::Hz250);
        assert_eq!(PollRate::from_sysfs_value(2).unwrap(), PollRate::Hz500);
        assert_eq!(PollRate::from_sysfs_value(1).unwrap(), PollRate::Hz1000);
        assert!(PollRate::from_sysfs_value(0).is_err());
        assert!(PollRate::from_sysfs_value(3).is_err());
    }

    #[test]
    fn test_sysfs_roundtrip_matches_hz() {
        for rate in [PollRate::Hz125, PollRate::Hz250, PollRate::Hz500, PollRate::Hz1000] {
            let ms = rate.to_sysfs_value().unwrap();
            assert_eq!(1000 / u32::from(ms), rate.to_hz());
            assert_eq!(PollRate::from_sysfs_value(ms).unwrap(), rate);
        }
    }

    #[test]
    fn test_hz_conversion() {
        assert_eq!(PollRate::from_hz(125).unwrap(), PollRate::Hz125);
        assert_eq!(PollRate::from_hz(250).unwrap(), PollRate::Hz250);
        assert_eq!(PollRate::from_hz(500).unwrap(), PollRate::Hz500);
        assert_eq!(PollRate::from_hz(1000).unwrap(), PollRate::Hz1000);
        assert_eq!(PollRate::from_hz(2000).unwrap(), PollRate::Hz2000);
        assert_eq!(PollRate::from_hz(4000).unwrap(), PollRate::Hz4000);
        assert!(PollRate::from_hz(8000).is_err());
        assert!(PollRate::from_hz(333).is_err());

        assert_eq!(PollRate::Hz125.to_hz(), 125);
        assert_eq!(PollRate::Hz250.to_hz(), 250);
        assert_eq!(PollRate::Hz500.to_hz(), 500);
        assert_eq!(PollRate::Hz1000.to_hz(), 1000);
        assert_eq!(PollRate::Hz2000.to_hz(), 2000);
        assert_eq!(PollRate::Hz4000.to_hz(), 4000);
    }

    #[test]
    fn test_hardware_support_check() {
        assert!(!PollRate::Hz125.requires_hardware_support());
        assert!(!PollRate::Hz250.requires_hardware_support());
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

    #[cfg(target_os = "linux")]
    #[test]
    fn test_device_type_paths() {
        assert_eq!(
            DeviceType::Mouse.sysfs_path(),
            "/sys/module/usbhid/parameters/mousepoll"
        );
        assert_eq!(
            DeviceType::Keyboard.sysfs_path(),
            "/sys/module/usbhid/parameters/kbpoll"
        );
    }
}

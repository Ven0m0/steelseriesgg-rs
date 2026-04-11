//! Apex Pro TKL 2023 specific implementations.

use super::{GenericKeyboard, Keyboard};
use crate::Result;
use crate::devices::hid_reports::{
    ActuationCommand, HidCommand, HidDeviceType, HidReportBuilder, KEYBOARD_REPORT_SIZE,
};
use crate::devices::key_mapping::{KeyAddress, KeyId, KeyMapping};
use crate::devices::product_ids;
use crate::devices::zone_mapping::{ZoneEffect, ZoneMapping};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::{Color, PerKeyEffect};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

/// Apex 2023 new protocol constants (from OpenRGB reverse engineering).
const APEX_2023_PACKET_LENGTH: usize = 643;
const APEX_2023_PACKET_ID_INIT: u8 = 0x4B;
const APEX_2023_PACKET_ID_DIRECT_WIRED: u8 = 0x40;
const APEX_2023_PACKET_ID_DIRECT_WIRELESS: u8 = 0x61;

/// TKL key HID codes (from OpenRGB SteelSeriesApexController.cpp).
const TKL_KEYS: &[u8] = &[
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
    0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D,
    0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50,
    0x51, 0x52, 0x64, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0, 0x31, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x53,
    0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63,
];

/// Apex Pro TKL 2023 implementation.
pub struct ApexProTkl2023 {
    inner: GenericKeyboard,
    initialized_new_protocol: bool,
}

impl ApexProTkl2023 {
    /// Product ID for Apex Pro TKL 2023.
    pub const PRODUCT_ID: u16 = 0x1628;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self {
            inner: keyboard,
            initialized_new_protocol: false,
        }
    }

    /// Create for wireless model without a hidapi device handle.
    /// Uses raw hidraw feature reports only.
    pub fn new_wireless_raw(info: DeviceInfo) -> Self {
        Self {
            inner: GenericKeyboard::new_without_device(info),
            initialized_new_protocol: false,
        }
    }

    /// Whether this device uses the new 2023 per-key direct protocol.
    /// Only the wireless variant uses this unconditionally; the wired model
    /// has its own path behind the `experimental-apex-2023` feature flag.
    fn uses_new_protocol(&self) -> bool {
        self.is_wireless()
    }

    /// Whether this is the wireless variant.
    fn is_wireless(&self) -> bool {
        self.inner.info().product_id == product_ids::APEX_PRO_TKL_2023_WIRELESS
    }

    /// Send the 0x4B initialization feature report required by new protocol.
    fn ensure_new_protocol_init(&mut self) -> Result<()> {
        if self.initialized_new_protocol {
            return Ok(());
        }
        let mut buf = vec![0u8; APEX_2023_PACKET_LENGTH];
        buf[0] = 0x00; // Report ID
        buf[1] = APEX_2023_PACKET_ID_INIT;
        self.inner.send_feature(&buf, APEX_2023_PACKET_LENGTH)?;
        self.initialized_new_protocol = true;
        tracing::info!("Sent Apex 2023 new protocol init (0x4B)");
        Ok(())
    }

    /// Send per-key direct RGB using the new 643-byte feature report protocol.
    fn send_direct_rgb_new_protocol(&mut self, color: Color) -> Result<()> {
        self.ensure_new_protocol_init()?;

        let packet_id = if self.is_wireless() {
            APEX_2023_PACKET_ID_DIRECT_WIRELESS
        } else {
            APEX_2023_PACKET_ID_DIRECT_WIRED
        };

        let num_keys = TKL_KEYS.len();
        let mut buf = vec![0u8; APEX_2023_PACKET_LENGTH];
        buf[0] = 0x00; // Report ID
        buf[1] = packet_id;
        buf[2] = num_keys as u8;

        for (i, &hid_code) in TKL_KEYS.iter().enumerate() {
            let offset = 3 + i * 4;
            buf[offset] = hid_code;
            buf[offset + 1] = color.r;
            buf[offset + 2] = color.g;
            buf[offset + 3] = color.b;
        }

        self.inner.send_feature(&buf, APEX_2023_PACKET_LENGTH)
    }

    /// Set actuation point for all keys (global).
    /// Value is in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm).
    /// Send actuation command and update cache
    fn send_actuation_command(&mut self, command: ActuationCommand) -> Result<()> {
        let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
        let size = report_builder.build_report(command.clone(), &mut buffer)?;
        self.inner.send_raw(&buffer[..size])?;
        self.inner.update_cached_actuation_point(command.actuation_point);
        Ok(())
    }

    pub fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        let command = ActuationCommand::new(value);
        command.validate()?;
        self.send_actuation_command(command)
    }

    /// Set actuation point in millimeters.
    /// Precision is limited to 0.1mm increments.
    pub fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        let command = ActuationCommand::from_mm(mm);
        command.validate()?;
        self.send_actuation_command(command)
    }

    #[cfg(feature = "experimental-apex-2023")]
    const fn experimental_direct_key_id(key_id: KeyId) -> Option<u8> {
        Some(match key_id {
            KeyId::A => 0x04,
            KeyId::B => 0x05,
            KeyId::C => 0x06,
            KeyId::D => 0x07,
            KeyId::E => 0x08,
            KeyId::F => 0x09,
            KeyId::G => 0x0A,
            KeyId::H => 0x0B,
            KeyId::I => 0x0C,
            KeyId::J => 0x0D,
            KeyId::K => 0x0E,
            KeyId::L => 0x0F,
            KeyId::M => 0x10,
            KeyId::N => 0x11,
            KeyId::O => 0x12,
            KeyId::P => 0x13,
            KeyId::Q => 0x14,
            KeyId::R => 0x15,
            KeyId::S => 0x16,
            KeyId::T => 0x17,
            KeyId::U => 0x18,
            KeyId::V => 0x19,
            KeyId::W => 0x1A,
            KeyId::X => 0x1B,
            KeyId::Y => 0x1C,
            KeyId::Z => 0x1D,
            KeyId::Key1 => 0x1E,
            KeyId::Key2 => 0x1F,
            KeyId::Key3 => 0x20,
            KeyId::Key4 => 0x21,
            KeyId::Key5 => 0x22,
            KeyId::Key6 => 0x23,
            KeyId::Key7 => 0x24,
            KeyId::Key8 => 0x25,
            KeyId::Key9 => 0x26,
            KeyId::Key0 => 0x27,
            KeyId::Enter => 0x28,
            KeyId::Escape => 0x29,
            KeyId::Backspace => 0x2A,
            KeyId::Tab => 0x2B,
            KeyId::Space => 0x2C,
            KeyId::Minus => 0x2D,
            KeyId::Equal => 0x2E,
            KeyId::LeftBracket => 0x2F,
            KeyId::RightBracket => 0x30,
            KeyId::Backslash => 0x31,
            KeyId::Semicolon => 0x33,
            KeyId::Quote => 0x34,
            KeyId::Backtick => 0x35,
            KeyId::Comma => 0x36,
            KeyId::Period => 0x37,
            KeyId::Slash => 0x38,
            KeyId::CapsLock => 0x39,
            KeyId::F1 => 0x3A,
            KeyId::F2 => 0x3B,
            KeyId::F3 => 0x3C,
            KeyId::F4 => 0x3D,
            KeyId::F5 => 0x3E,
            KeyId::F6 => 0x3F,
            KeyId::F7 => 0x40,
            KeyId::F8 => 0x41,
            KeyId::F9 => 0x42,
            KeyId::F10 => 0x43,
            KeyId::F11 => 0x44,
            KeyId::F12 => 0x45,
            KeyId::Insert => 0x49,
            KeyId::Home => 0x4A,
            KeyId::PageUp => 0x4B,
            KeyId::Delete => 0x4C,
            KeyId::End => 0x4D,
            KeyId::PageDown => 0x4E,
            KeyId::ArrowRight => 0x4F,
            KeyId::ArrowLeft => 0x50,
            KeyId::ArrowDown => 0x51,
            KeyId::ArrowUp => 0x52,
            KeyId::Menu => 0x65,
            KeyId::LeftCtrl => 0xE0,
            KeyId::LeftShift => 0xE1,
            KeyId::LeftAlt => 0xE2,
            KeyId::LeftWin => 0xE3,
            KeyId::RightCtrl => 0xE4,
            KeyId::RightShift => 0xE5,
            KeyId::RightAlt => 0xE6,
            KeyId::RightWin => 0xE7,
            KeyId::SteelSeriesKey | KeyId::VolumeWheel => return None,
            KeyId::NumLock
            | KeyId::NumSlash
            | KeyId::NumAsterisk
            | KeyId::NumMinus
            | KeyId::Num7
            | KeyId::Num8
            | KeyId::Num9
            | KeyId::NumPlus
            | KeyId::Num4
            | KeyId::Num5
            | KeyId::Num6
            | KeyId::Num1
            | KeyId::Num2
            | KeyId::Num3
            | KeyId::NumEnter
            | KeyId::Num0
            | KeyId::NumPeriod => return None,
        })
    }

    #[cfg(feature = "experimental-apex-2023")]
    fn build_experimental_direct_command(
        &self,
        key_colors: &[(KeyId, Color)],
    ) -> Option<crate::devices::hid_reports::Apex2023DirectCommand> {
        use crate::devices::hid_reports::Apex2023DirectCommand;

        if key_colors.is_empty() {
            return None;
        }

        let mut command = Apex2023DirectCommand::new();
        for (key_id, color) in key_colors {
            let Some(direct_key_id) = Self::experimental_direct_key_id(*key_id) else {
                tracing::debug!(
                    "Falling back to placeholder Apex per-key path for unsupported experimental key {:?}",
                    key_id
                );
                return None;
            };
            command.set_key_color(direct_key_id, *color);
        }

        Some(command)
    }
}

// Delegate Device trait
impl Device for ApexProTkl2023 {
    fn info(&self) -> &DeviceInfo {
        self.inner.info()
    }

    fn device_type(&self) -> DeviceType {
        self.inner.device_type()
    }

    fn initialize(&mut self) -> Result<()> {
        // Wireless variant uses feature reports for init, not output reports.
        // The 0x4B init is sent lazily in ensure_new_protocol_init().
        if self.is_wireless() {
            return Ok(());
        }
        self.inner.initialize()
    }

    fn close(&mut self) -> Result<()> {
        self.inner.close()
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.inner.send_raw(data)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.receive_raw(buf)
    }
}

// Delegate Keyboard trait
#[async_trait]
impl Keyboard for ApexProTkl2023 {
    async fn set_color(&mut self, color: Color) -> Result<()> {
        if self.uses_new_protocol() {
            return self.send_direct_rgb_new_protocol(color);
        }
        self.inner.set_color(color).await
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        if self.uses_new_protocol() {
            // For zone colors, just use the first color for all keys via direct protocol
            let color = colors.first().copied().unwrap_or(Color::BLACK);
            return self.send_direct_rgb_new_protocol(color);
        }
        self.inner.set_zone_colors(colors).await
    }

    fn zone_count(&self) -> usize {
        self.inner.zone_count()
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.inner.set_brightness(brightness).await
    }

    async fn apply(&mut self) -> Result<()> {
        self.inner.apply().await
    }

    fn supports_per_key_rgb(&self) -> bool {
        self.inner.supports_per_key_rgb()
    }

    fn get_key_mapping(&self) -> Option<&KeyMapping> {
        self.inner.get_key_mapping()
    }

    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        self.set_key_colors(&[(key_id, color)]).await
    }

    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        #[cfg(feature = "experimental-apex-2023")]
        if let Some(command) = self.build_experimental_direct_command(key_colors) {
            let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);
            let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
            let size = report_builder.build_report(command, &mut buffer)?;
            return self.inner.send_raw(&buffer[..size]);
        }

        self.inner.set_key_colors(key_colors).await
    }

    async fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()> {
        self.inner.set_key_color_direct(address, color).await
    }

    async fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        self.inner.set_key_colors_direct(key_colors).await
    }

    async fn clear_per_key_rgb(&mut self) -> Result<()> {
        self.inner.clear_per_key_rgb().await
    }

    async fn set_key_region(&mut self, start_hid: u8, count: u8, color: Color) -> Result<()> {
        self.inner.set_key_region(start_hid, count, color).await
    }

    fn get_zone_mapping(&self) -> Option<&ZoneMapping> {
        self.inner.get_zone_mapping()
    }

    async fn set_zone_effect(&mut self, effect: ZoneEffect) -> Result<()> {
        self.inner.set_zone_effect(effect).await
    }

    async fn simulate_per_key_with_zones(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        self.inner.simulate_per_key_with_zones(key_colors).await
    }

    async fn set_zone_colors_with_retry(&mut self, colors: &[Color], max_retries: usize) -> Result<()> {
        self.inner.set_zone_colors_with_retry(colors, max_retries).await
    }

    async fn test_zone_reliability(&mut self) -> Result<Vec<bool>> {
        self.inner.test_zone_reliability().await
    }

    fn supports_per_key_effects(&self) -> bool {
        self.inner.supports_per_key_effects()
    }

    async fn set_per_key_effect(&mut self, effect: PerKeyEffect) -> Result<()> {
        self.inner.set_per_key_effect(effect).await
    }

    fn get_per_key_effect(&self) -> Option<&PerKeyEffect> {
        self.inner.get_per_key_effect()
    }

    async fn trigger_key_reactive(&mut self, keys: &[KeyId], duration: f32) -> Result<()> {
        self.inner.trigger_key_reactive(keys, duration).await
    }

    async fn apply_per_key_effect_with_brightness(&mut self, brightness: f32) -> Result<()> {
        self.inner.apply_per_key_effect_with_brightness(brightness).await
    }

    async fn convert_per_key_to_zones(&mut self, effect: &PerKeyEffect) -> Result<()> {
        self.inner.convert_per_key_to_zones(effect).await
    }

    fn get_rgb_performance_stats(&self) -> Option<&crate::performance::PerformanceStats> {
        self.inner.get_rgb_performance_stats()
    }

    fn get_optimal_frame_time(&self) -> Option<std::time::Duration> {
        self.inner.get_optimal_frame_time()
    }

    fn cleanup_rgb_caches(&mut self) {
        self.inner.cleanup_rgb_caches()
    }

    fn set_performance_optimization(&mut self, enabled: bool) {
        self.inner.set_performance_optimization(enabled)
    }

    fn read_actuation_point(&mut self) -> Result<u8> {
        // Implement read if command known, otherwise delegate to inner which returns error
        self.inner.read_actuation_point()
    }

    fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        self.set_actuation_point(value)
    }

    fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        self.set_actuation_point_mm(mm)
    }
}

// Deref allows access to Device trait methods like send_raw/receive_raw
impl Deref for ApexProTkl2023 {
    type Target = GenericKeyboard;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ApexProTkl2023 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

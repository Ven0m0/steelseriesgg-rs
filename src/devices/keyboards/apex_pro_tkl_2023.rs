//! Apex Pro TKL 2023 specific implementations.

use super::{GenericKeyboard, Keyboard};
use crate::Result;
use crate::devices::hid_reports::{ActuationCommand, HidCommand, HidDeviceType, HidReportBuilder};
use crate::devices::key_mapping::{KeyAddress, KeyId, KeyMapping};
use crate::devices::zone_mapping::{ZoneEffect, ZoneMapping};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::{Color, PerKeyEffect};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

/// Apex Pro TKL 2023 implementation.
pub struct ApexProTkl2023 {
    inner: GenericKeyboard,
}

impl ApexProTkl2023 {
    /// Product ID for Apex Pro TKL 2023.
    pub const PRODUCT_ID: u16 = 0x1628;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self { inner: keyboard }
    }

    /// Set actuation point for all keys (global).
    /// Value is in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm).
    pub fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        // Create ActuationCommand and validate it
        let command = ActuationCommand::new(value);
        command.validate()?;

        // Use the new command infrastructure for consistent serialization
        let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);

        let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
        let size = report_builder.build_report(command, &mut buffer)?;

        // Use send_raw from inner Device trait
        self.inner.send_raw(&buffer[..size])
    }

    /// Set actuation point in millimeters.
    /// Precision is limited to 0.1mm increments.
    pub fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        let command = ActuationCommand::from_mm(mm);
        command.validate()?;

        // Use the new command infrastructure for consistent serialization
        let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);

        let mut buffer = [0u8; 65];
        let size = report_builder.build_report(command, &mut buffer)?;

        // Use send_raw from inner Device trait
        self.inner.send_raw(&buffer[..size])
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
    fn set_color(&mut self, color: Color) -> Result<()> {
        self.inner.set_color(color)
    }

    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.inner.set_zone_colors(colors)
    }

    fn zone_count(&self) -> usize {
        self.inner.zone_count()
    }

    fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.inner.set_brightness(brightness)
    }

    fn apply(&mut self) -> Result<()> {
        self.inner.apply()
    }

    fn supports_per_key_rgb(&self) -> bool {
        self.inner.supports_per_key_rgb()
    }

    fn get_key_mapping(&self) -> Option<&KeyMapping> {
        self.inner.get_key_mapping()
    }

    fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        self.inner.set_key_color(key_id, color)
    }

    fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        self.inner.set_key_colors(key_colors)
    }

    fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()> {
        self.inner.set_key_color_direct(address, color)
    }

    fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        self.inner.set_key_colors_direct(key_colors)
    }

    fn clear_per_key_rgb(&mut self) -> Result<()> {
        self.inner.clear_per_key_rgb()
    }

    fn set_key_region(&mut self, start_row: u8, start_col: u8, rows: u8, cols: u8, color: Color) -> Result<()> {
        self.inner.set_key_region(start_row, start_col, rows, cols, color)
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

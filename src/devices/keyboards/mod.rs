//! Keyboard device support (Apex series).

pub mod apex;
pub mod apex_pro_tkl_2023;

use super::diagnostics::{HidOperation, with_global_diagnostics};
use super::hid_reports::{
    ApplyCommand, BrightnessCommand, HidDeviceType, HidReportBuilder, PerKeyRgbBuilder, PerKeyRgbCommand,
    RgbZoneCommand,
};
use super::key_mapping::{KeyAddress, KeyId, KeyMapping, KeyMappingDatabase};
use super::zone_mapping::{ZoneEffect, ZoneFallback, ZoneMapping as ZoneMap};
use super::{Device, DeviceInfo, DeviceType, write_padded_report, zone_count_for_product_id};
use crate::rgb::{Color, PerKeyEffect, PerKeyRgbController};
use crate::{Error, Result};
use async_trait::async_trait;
use hidapi::HidDevice;
use parking_lot::Mutex;
use std::sync::Arc;

/// Trait for keyboard-specific functionality.
#[async_trait]
pub trait Keyboard: Device {
    /// Set the entire keyboard to a single color.
    async fn set_color(&mut self, color: Color) -> Result<()>;

    /// Set colors for individual zones.
    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()>;

    /// Get the number of RGB zones.
    fn zone_count(&self) -> usize;

    /// Set keyboard brightness (0-100).
    async fn set_brightness(&mut self, brightness: u8) -> Result<()>;

    /// Apply the current RGB settings.
    async fn apply(&mut self) -> Result<()>;

    // === Per-Key RGB Control ===

    /// Check if per-key RGB control is supported by this keyboard.
    fn supports_per_key_rgb(&self) -> bool;

    /// Get the key mapping for this keyboard (if available).
    fn get_key_mapping(&self) -> Option<&KeyMapping>;

    /// Set RGB color for a specific key by logical key ID.
    ///
    /// Uses the keyboard's key mapping to convert logical key IDs to matrix addresses.
    /// Returns an error if the key is not found in the mapping or per-key RGB is not supported.
    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()>;

    /// Set RGB colors for multiple keys by logical key IDs.
    ///
    /// Uses the keyboard's key mapping to convert logical key IDs to matrix addresses.
    /// Keys not found in the mapping are ignored with a warning.
    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()>;

    /// Set RGB color for a specific key by direct matrix address.
    ///
    /// Bypasses key mapping and directly addresses the key matrix.
    /// Use with caution - invalid addresses may cause device issues.
    async fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()>;

    /// Set RGB colors for multiple keys by direct matrix addresses.
    ///
    /// Bypasses key mapping and directly addresses the key matrix.
    /// Use with caution - invalid addresses may cause device issues.
    async fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()>;

    /// Set all keys to black (turn off per-key RGB).
    async fn clear_per_key_rgb(&mut self) -> Result<()>;

    /// Set a region of keys to the same color using HID codes.
    async fn set_key_region(&mut self, start_hid: u8, count: u8, color: Color) -> Result<()>;

    // === Zone-based RGB Fallback ===

    /// Get zone mapping information for this keyboard.
    fn get_zone_mapping(&self) -> Option<&ZoneMap>;

    /// Set zone-based RGB effect as fallback.
    async fn set_zone_effect(&mut self, effect: ZoneEffect) -> Result<()>;

    /// Simulate per-key effect using zone-based fallback.
    async fn simulate_per_key_with_zones(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()>;

    /// Enhanced zone-based RGB with retry logic.
    async fn set_zone_colors_with_retry(&mut self, colors: &[Color], max_retries: usize) -> Result<()>;

    /// Test zone connectivity and reliability.
    async fn test_zone_reliability(&mut self) -> Result<Vec<bool>>;

    // === Per-Key RGB Effect Support ===

    /// Check if per-key RGB effects are supported.
    fn supports_per_key_effects(&self) -> bool;

    /// Set per-key RGB effect.
    async fn set_per_key_effect(&mut self, effect: PerKeyEffect) -> Result<()>;

    /// Get current per-key RGB effect (if available).
    fn get_per_key_effect(&self) -> Option<&PerKeyEffect>;

    /// Trigger reactive effect for specific keys.
    async fn trigger_key_reactive(&mut self, keys: &[KeyId], duration: f32) -> Result<()>;

    /// Apply per-key effect with brightness control.
    async fn apply_per_key_effect_with_brightness(&mut self, brightness: f32) -> Result<()>;

    /// Convert per-key effect to zone-based fallback.
    async fn convert_per_key_to_zones(&mut self, effect: &PerKeyEffect) -> Result<()>;

    // === Performance Optimization ===

    /// Get current performance statistics for RGB operations.
    fn get_rgb_performance_stats(&self) -> Option<&crate::performance::PerformanceStats>;

    /// Get optimal frame time for current adaptive refresh rate.
    fn get_optimal_frame_time(&self) -> Option<std::time::Duration>;

    /// Force cleanup of performance caches.
    fn cleanup_rgb_caches(&mut self);

    /// Enable/disable performance optimizations.
    fn set_performance_optimization(&mut self, enabled: bool);

    /// Read current actuation point setting from keyboard (if supported).
    ///
    /// **NOTE**: This is currently a placeholder. The HID command to read
    /// actuation settings has not yet been discovered. This function always
    /// returns an error indicating the feature is not implemented.
    ///
    /// Returns actuation point in 0.1mm units (e.g., 4 = 0.4mm, 36 = 3.6mm).
    fn read_actuation_point(&mut self) -> Result<u8>;

    /// Set actuation point for adjustable actuation keyboards (if supported).
    ///
    /// Value is in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm).
    /// Returns an error if the keyboard doesn't support adjustable actuation.
    fn set_actuation_point(&mut self, value: u8) -> Result<()>;

    /// Set actuation point for adjustable actuation keyboards in millimeters (if supported).
    ///
    /// Value is in millimeters with precision limited to 0.1mm increments.
    /// Returns an error if the keyboard doesn't support adjustable actuation.
    fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()>;
}

/// Generic SteelSeries keyboard implementation.
pub struct GenericKeyboard {
    info: DeviceInfo,
    device: Option<Arc<Mutex<HidDevice>>>,
    zone_count: usize,
    report_builder: HidReportBuilder,
    key_mapping: Option<KeyMapping>,
    zone_fallback: ZoneFallback,
    zone_mapping: Option<ZoneMap>,
    per_key_controller: Option<PerKeyRgbController>,
    zone_color_buffer: Vec<Color>,
    actuation_point_cache: Option<u8>,
}

impl GenericKeyboard {
    /// Create a new keyboard instance.
    pub fn new(info: DeviceInfo, device: HidDevice) -> Self {
        let zone_count = zone_count_for_product_id(info.product_id);

        let key_mapping = KeyMappingDatabase::new().get_mapping(info.product_id).cloned();

        if key_mapping.is_some() {
            tracing::debug!("Loaded key mapping for product ID 0x{:04x}", info.product_id);
        } else {
            tracing::warn!(
                "No key mapping available for product ID 0x{:04x} - per-key RGB disabled",
                info.product_id
            );
        }

        let zone_fallback = ZoneFallback::new();
        let zone_mapping = zone_fallback.get_mapping(info.product_id).cloned();

        if zone_mapping.is_some() {
            tracing::debug!("Loaded zone mapping for product ID 0x{:04x}", info.product_id);
        } else {
            tracing::warn!(
                "No zone mapping available for product ID 0x{:04x} - using basic zone fallback",
                info.product_id
            );
        }

        let per_key_controller = key_mapping.as_ref().map(|mapping| {
            tracing::debug!(
                "Initializing per-key RGB controller for product ID 0x{:04x}",
                info.product_id
            );
            PerKeyRgbController::new_with_performance(mapping.clone())
        });

        Self {
            info,
            device: Some(Arc::new(Mutex::new(device))),
            zone_count,
            report_builder: HidReportBuilder::new(HidDeviceType::Keyboard),
            key_mapping,
            zone_fallback,
            zone_mapping,
            per_key_controller,
            zone_color_buffer: Vec::with_capacity(zone_count),
            actuation_point_cache: None,
        }
    }

    /// Create without a HID device handle (for wireless raw-only mode).
    pub fn new_without_device(info: DeviceInfo) -> Self {
        let zone_count = zone_count_for_product_id(info.product_id);
        let key_mapping = KeyMappingDatabase::new().get_mapping(info.product_id).cloned();
        let zone_fallback = ZoneFallback::new();
        let zone_mapping = zone_fallback.get_mapping(info.product_id).cloned();
        let per_key_controller = key_mapping.as_ref().map(|mapping| {
            PerKeyRgbController::new_with_performance(mapping.clone())
        });

        Self {
            info,
            device: None,
            zone_count,
            report_builder: HidReportBuilder::new(HidDeviceType::Keyboard),
            key_mapping,
            zone_fallback,
            zone_mapping,
            per_key_controller,
            zone_color_buffer: Vec::with_capacity(zone_count),
            actuation_point_cache: None,
        }
    }

    /// Send a HID report to the keyboard synchronously (blocking).
    fn send_report(&mut self, data: &[u8]) -> Result<()> {
        use tracing::debug;

        with_global_diagnostics(|diag| {
            if !diag.validate_report_structure(data) {
                debug!("HID report validation failed, sending anyway");
            }
        });

        debug!("Sending HID report ({} bytes): {:02x?}", data.len(), data);

        let device = self
            .device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))?;
        let device = device.lock();

        let result = if let Some(result) = with_global_diagnostics(|diag| {
            diag.record_timed_operation(HidOperation::Send, data, || {
                write_padded_report(&device, data, 65, true)
            })
        }) {
            result
        } else {
            write_padded_report(&device, data, 65, true)
        };

        if result.is_ok() {
            debug!("HID report sent successfully");
        } else if let Err(e) = &result {
            debug!("HID report failed: {:?}", e);
        }

        result
    }

    /// Send a HID report to the keyboard asynchronously (non-blocking).
    ///
    /// Offloads the blocking hidapi write to `spawn_blocking` so the tokio
    /// worker thread is not stalled during HID I/O.
    async fn send_report_async(&self, data: &[u8]) -> Result<()> {
        use tracing::debug;

        with_global_diagnostics(|diag| {
            if !diag.validate_report_structure(data) {
                debug!("HID report validation failed, sending anyway");
            }
        });

        debug!("Sending HID report ({} bytes): {:02x?}", data.len(), data);

        let device = self
            .device
            .clone()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))?;

        let data = data.to_vec();

        let result = tokio::task::spawn_blocking(move || {
            let device = device.lock();

            if let Some(result) = with_global_diagnostics(|diag| {
                diag.record_timed_operation(HidOperation::Send, &data, || {
                    write_padded_report(&device, &data, 65, true)
                })
            }) {
                result
            } else {
                write_padded_report(&device, &data, 65, true)
            }
        })
        .await
        .map_err(|e| {
            if e.is_cancelled() {
                Error::DeviceCommunication(format!("HID write task was cancelled: {}", e))
            } else {
                Error::DeviceCommunication(format!("HID write task failed: {}", e))
            }
        })?;

        match &result {
            Ok(_) => debug!("HID report sent successfully"),
            Err(e) => debug!("HID report failed: {:?}", e),
        }

        result
    }

    /// Send a HID feature report of arbitrary size (for Apex 2023 new protocol).
    /// Uses direct ioctl to bypass hidapi's broken HIDIOCSFEATURE direction bits.
    /// Resolves the correct hidraw path for the control interface (interface 3 for wireless).
    /// Resolves the correct hidraw path for the control interface (interface 3 for wireless).
    #[cfg(unix)]
    pub fn send_feature(&self, data: &[u8], report_len: usize) -> Result<()> {
        let path = super::find_hidraw_for_interface(self.info.vendor_id, self.info.product_id, 3)
            .unwrap_or_else(|| self.info.path.clone());
        super::send_feature_report_raw(&path, data, report_len)
    }

    /// Send a HID feature report of arbitrary size (for Apex 2023 new protocol).
    ///
    /// On non-Unix platforms, raw hidraw feature reports are not supported and this
    /// method will always return a platform-not-supported error.
    #[cfg(not(unix))]
    pub fn send_feature(&self, _data: &[u8], _report_len: usize) -> Result<()> {
        Err(Error::PlatformNotSupported(
            "Raw HID feature reports are only supported on Unix-like platforms",
        ))
    }
    #[cfg(not(unix))]
    pub fn send_feature(&self, _data: &[u8], _report_len: usize) -> Result<()> {
        Err(Error::DeviceCommunication(
            "Raw HID feature reports are only supported on Unix platforms".to_string(),
        ))
    }

    async fn send_zone_buffer_async(&mut self) -> Result<()> {
        // Wireless keyboards (e.g. Apex Pro TKL 2023 Wireless, PID 0x1632) don't
        // support the 0xFF "all zones" selector. Send per-zone commands instead,
        // with zone indices starting at 1.
        if self.info.product_id == super::product_ids::APEX_PRO_TKL_2023_WIRELESS {
            for i in 0..self.zone_color_buffer.len() {
                let color = self.zone_color_buffer[i];
                let zone_index = (i + 1) as u8; // zones start at 1, not 0
                let rgb_command = RgbZoneCommand::new_specific_zone(zone_index, color);
                let mut buffer = [0u8; super::KEYBOARD_REPORT_SIZE];
                let size = self.report_builder.build_report(rgb_command, &mut buffer)?;
                self.send_report_async(&buffer[..size]).await?;
            }
            return Ok(());
        }

        let rgb_command = RgbZoneCommand::new_all_zones(&self.zone_color_buffer);
        let mut buffer = [0u8; super::KEYBOARD_REPORT_SIZE];
        let size = self.report_builder.build_report(rgb_command, &mut buffer)?;
        self.send_report_async(&buffer[..size]).await
    }

    /// Update the cached actuation point value.
    ///
    /// This should be called by wrapper structs (like ApexProTkl2023) when they
    /// successfully set the actuation point, so that `read_actuation_point` can
    /// return the last known value.
    pub fn update_cached_actuation_point(&mut self, value: u8) {
        self.actuation_point_cache = Some(value);
    }
}

impl Device for GenericKeyboard {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Keyboard
    }

    fn initialize(&mut self) -> Result<()> {
        let mut buffer = [0u8; 65];
        let size = self.report_builder.build_report(ApplyCommand, &mut buffer)?;
        self.send_report(&buffer[..size])?;
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.device = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.send_report(data)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        let device = self
            .device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))?;
        let device = device.lock();

        if let Some(result) = with_global_diagnostics(|diag| {
            diag.record_timed_operation(HidOperation::Receive, &[], || {
                let len = device.read(buf)?;
                Ok((len, buf[..len].to_vec()))
            })
        }) {
            result.map(|(len, _data)| len)
        } else {
            let len = device.read(buf)?;
            Ok(len)
        }
    }
}

impl GenericKeyboard {
    fn compute_average_color(&self, key_colors: &[(KeyId, Color)]) -> Color {
        if key_colors.is_empty() {
            return Color::BLACK;
        }

        let mut total_r = 0u32;
        let mut total_g = 0u32;
        let mut total_b = 0u32;

        for (_, color) in key_colors {
            total_r += color.r as u32;
            total_g += color.g as u32;
            total_b += color.b as u32;
        }

        let count = key_colors.len() as u32;
        Color::new(
            (total_r / count) as u8,
            (total_g / count) as u8,
            (total_b / count) as u8,
        )
    }
}

#[async_trait]
impl Keyboard for GenericKeyboard {
    async fn set_color(&mut self, color: Color) -> Result<()> {
        self.zone_color_buffer.clear();
        self.zone_color_buffer.resize(self.zone_count, color);
        self.send_zone_buffer_async().await
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.zone_color_buffer.clear();
        let len = colors.len().min(self.zone_count);
        self.zone_color_buffer.extend_from_slice(&colors[..len]);

        while self.zone_color_buffer.len() < self.zone_count {
            self.zone_color_buffer.push(Color::BLACK);
        }

        self.send_zone_buffer_async().await
    }

    fn zone_count(&self) -> usize {
        self.zone_count
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        let brightness_command = BrightnessCommand::new(brightness);
        let mut buffer = [0u8; 65];
        let size = self.report_builder.build_report(brightness_command, &mut buffer)?;
        self.send_report_async(&buffer[..size]).await
    }

    async fn apply(&mut self) -> Result<()> {
        let apply_command = ApplyCommand;
        let mut buffer = [0u8; 65];
        let size = self.report_builder.build_report(apply_command, &mut buffer)?;
        let _ = self.send_report_async(&buffer[..size]).await;
        Ok(())
    }

    // === Per-Key RGB Control Implementation ===

    fn supports_per_key_rgb(&self) -> bool {
        self.key_mapping.is_some()
    }

    fn get_key_mapping(&self) -> Option<&KeyMapping> {
        self.key_mapping.as_ref()
    }

    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        let mapping = self.key_mapping.as_ref().ok_or_else(|| {
            Error::DeviceCommunication("Per-key RGB not supported - no key mapping available".to_string())
        })?;

        let address = mapping
            .get_key_address(key_id)
            .ok_or_else(|| Error::DeviceCommunication(format!("Key {:?} not found in key mapping", key_id)))?;

        self.set_key_color_direct(address, color).await
    }

    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        let mapping = self.key_mapping.as_ref().ok_or_else(|| {
            Error::DeviceCommunication("Per-key RGB not supported - no key mapping available".to_string())
        })?;

        let mut builder = PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::HidCode);
        for (key_id, color) in key_colors {
            if let Some(address) = mapping.get_key_address(*key_id) {
                builder.add_key_matrix(address, *color);
            } else {
                tracing::warn!("Key {:?} not found in key mapping", key_id);
            }
        }

        if builder.is_empty() {
            return Err(Error::DeviceCommunication(
                "No valid keys found in key mapping".to_string(),
            ));
        }

        let command = builder.build();
        let mut buffer = [0u8; 65];
        let size = self.report_builder.build_report(command, &mut buffer)?;
        self.send_report_async(&buffer[..size]).await
    }

    async fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()> {
        let command = PerKeyRgbCommand::single_key(address, color);
        let mut buffer = [0u8; 65];
        let size = self.report_builder.build_report(command, &mut buffer)?;
        self.send_report_async(&buffer[..size]).await
    }

    async fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        if key_colors.is_empty() {
            return Err(Error::DeviceCommunication("No key colors provided".to_string()));
        }

        let mut builder = PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::HidCode);

        for (address, color) in key_colors {
            builder.add_key_matrix(*address, *color);
        }

        let command = builder.build();
        let mut buffer = [0u8; 65];
        let size = self.report_builder.build_report(command, &mut buffer)?;
        self.send_report_async(&buffer[..size]).await
    }

    async fn clear_per_key_rgb(&mut self) -> Result<()> {
        if let Some(ref mapping) = self.key_mapping {
            let black_keys: Vec<(KeyId, Color)> = mapping
                .get_all_keys()
                .iter()
                .map(|&key_id| (key_id, Color::BLACK))
                .collect();

            if !black_keys.is_empty() {
                self.set_key_colors(&black_keys).await
            } else {
                Ok(())
            }
        } else {
            let mut builder = PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::HidCode);

            for hid_code in 0..120 {
                builder.add_key_matrix(KeyAddress::new(hid_code), Color::BLACK);
            }

            let command = builder.build();
            let mut buffer = [0u8; 65];
            let size = self.report_builder.build_report(command, &mut buffer)?;
            self.send_report_async(&buffer[..size]).await
        }
    }

    async fn set_key_region(&mut self, start_hid: u8, count: u8, color: Color) -> Result<()> {
        let mut builder = PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::HidCode);
        builder.set_region(start_hid, count, color);

        if builder.is_empty() {
            return Err(Error::DeviceCommunication(
                "Invalid region - no keys to set".to_string(),
            ));
        }

        let command = builder.build();
        let mut buffer = [0u8; 65];
        let size = self.report_builder.build_report(command, &mut buffer)?;
        self.send_report_async(&buffer[..size]).await
    }

    // === Zone-based RGB Fallback Implementation ===

    fn get_zone_mapping(&self) -> Option<&ZoneMap> {
        self.zone_mapping.as_ref()
    }

    async fn set_zone_effect(&mut self, effect: ZoneEffect) -> Result<()> {
        self.zone_fallback.set_current_effect(effect.clone());
        let colors = effect.compute_colors(self.zone_count, 0.0);
        self.set_zone_colors_with_retry(&colors, 3).await
    }

    async fn simulate_per_key_with_zones(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        if let Some(zone_colors) = self
            .zone_fallback
            .simulate_per_key_effect(self.info.product_id, key_colors)
        {
            self.set_zone_colors_with_retry(&zone_colors, 3).await
        } else if !key_colors.is_empty() {
            let avg_color = self.compute_average_color(key_colors);
            self.set_color(avg_color).await?;
            self.apply().await
        } else {
            self.set_color(Color::BLACK).await?;
            self.apply().await
        }
    }

    async fn set_zone_colors_with_retry(&mut self, colors: &[Color], max_retries: usize) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.set_zone_colors(colors).await {
                Ok(()) => {
                    if attempt > 0 {
                        tracing::info!("Zone RGB succeeded on attempt {}", attempt + 1);
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries - 1 {
                        tracing::warn!("Zone RGB attempt {} failed, retrying: {:?}", attempt + 1, last_error);
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        }

        if let Some(e) = last_error {
            tracing::error!("Zone RGB failed after {} attempts", max_retries);
            Err(e)
        } else {
            Err(Error::DeviceCommunication(
                "Zone RGB failed with unknown error".to_string(),
            ))
        }
    }

    async fn test_zone_reliability(&mut self) -> Result<Vec<bool>> {
        let mut results = Vec::new();

        for zone_index in 0..self.zone_count {
            let mut zone_colors = vec![Color::BLACK; self.zone_count];
            zone_colors[zone_index] = Color::WHITE;

            let success = self.set_zone_colors(&zone_colors).await.is_ok();
            results.push(success);

            if !success {
                tracing::warn!("Zone {} failed reliability test", zone_index);
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        let _ = self.set_color(Color::BLACK).await;

        tracing::info!(
            "Zone reliability test completed: {}/{} zones working",
            results.iter().filter(|&&x| x).count(),
            results.len()
        );

        Ok(results)
    }

    // === Per-Key RGB Effect Implementation ===

    fn supports_per_key_effects(&self) -> bool {
        self.per_key_controller.is_some()
    }

    async fn set_per_key_effect(&mut self, effect: PerKeyEffect) -> Result<()> {
        let key_colors = if let Some(ref mut controller) = self.per_key_controller {
            controller.set_effect(effect.clone());
            Some(controller.compute_key_colors().to_vec())
        } else {
            None
        };

        if let Some(colors) = key_colors {
            self.set_key_colors(&colors).await?;
            self.apply().await
        } else {
            self.convert_per_key_to_zones(&effect).await
        }
    }

    fn get_per_key_effect(&self) -> Option<&PerKeyEffect> {
        self.per_key_controller.as_ref().map(|c| c.effect())
    }

    async fn trigger_key_reactive(&mut self, keys: &[KeyId], duration: f32) -> Result<()> {
        let key_colors = if let Some(ref mut controller) = self.per_key_controller {
            controller.trigger_reactive(keys, duration);
            Some(controller.compute_key_colors().to_vec())
        } else {
            None
        };

        if let Some(colors) = key_colors {
            self.set_key_colors(&colors).await?;
            self.apply().await
        } else if !keys.is_empty() {
            self.simulate_per_key_with_zones(&keys.iter().map(|&k| (k, Color::WHITE)).collect::<Vec<_>>())
                .await
        } else {
            Ok(())
        }
    }

    async fn apply_per_key_effect_with_brightness(&mut self, brightness: f32) -> Result<()> {
        let key_colors = if let Some(ref mut controller) = self.per_key_controller {
            controller.set_brightness(brightness.clamp(0.0, 1.0));
            Some(controller.compute_key_colors().to_vec())
        } else {
            None
        };

        if let Some(colors) = key_colors {
            self.set_key_colors(&colors).await?;
            self.apply().await
        } else {
            self.set_brightness((brightness * 100.0) as u8).await?;
            self.apply().await
        }
    }

    async fn convert_per_key_to_zones(&mut self, effect: &PerKeyEffect) -> Result<()> {
        let zone_effect = match effect {
            PerKeyEffect::Static { color } => ZoneEffect::Solid(*color),

            PerKeyEffect::Breathing { color, speed: _ } => ZoneEffect::Breathing {
                colors: vec![*color],
                phase_offset: 0.0,
            },

            PerKeyEffect::Spectrum { speed: _ } => ZoneEffect::Wave {
                colors: vec![
                    Color::RED,
                    Color::ORANGE,
                    Color::YELLOW,
                    Color::GREEN,
                    Color::CYAN,
                    Color::BLUE,
                    Color::PURPLE,
                    Color::MAGENTA,
                ],
                offset: 0.0,
            },

            PerKeyEffect::Wave {
                colors,
                speed: _,
                direction: _,
            } => {
                if colors.is_empty() {
                    ZoneEffect::Solid(Color::BLACK)
                } else {
                    ZoneEffect::Wave {
                        colors: colors.clone(),
                        offset: 0.0,
                    }
                }
            }

            PerKeyEffect::Gradient {
                start,
                end,
                direction: _,
            } => ZoneEffect::Gradient {
                start: *start,
                end: *end,
            },

            PerKeyEffect::GameZone {
                wasd_color,
                default_color,
                ..
            } => ZoneEffect::Alternating(vec![*wasd_color, *default_color]),

            PerKeyEffect::Custom { key_colors } => {
                if key_colors.is_empty() {
                    ZoneEffect::Solid(Color::BLACK)
                } else {
                    let mut total_r = 0u32;
                    let mut total_g = 0u32;
                    let mut total_b = 0u32;
                    let count = key_colors.len() as u32;

                    for color in key_colors.values() {
                        total_r += color.r as u32;
                        total_g += color.g as u32;
                        total_b += color.b as u32;
                    }

                    let avg_color = Color::new(
                        (total_r / count) as u8,
                        (total_g / count) as u8,
                        (total_b / count) as u8,
                    );
                    ZoneEffect::Solid(avg_color)
                }
            }

            PerKeyEffect::Off => ZoneEffect::Solid(Color::BLACK),

            _ => ZoneEffect::Solid(Color::WHITE),
        };

        self.set_zone_effect(zone_effect).await
    }

    // === Performance Optimization Implementation ===

    fn get_rgb_performance_stats(&self) -> Option<&crate::performance::PerformanceStats> {
        self.per_key_controller.as_ref().and_then(|c| c.get_performance_stats())
    }

    fn get_optimal_frame_time(&self) -> Option<std::time::Duration> {
        self.per_key_controller.as_ref().and_then(|c| c.get_frame_time())
    }

    fn cleanup_rgb_caches(&mut self) {
        if let Some(ref mut controller) = self.per_key_controller {
            controller.cleanup_performance_caches();
        }
    }

    fn set_performance_optimization(&mut self, enabled: bool) {
        if let Some(ref mut controller) = self.per_key_controller {
            if enabled {
                controller.enable_performance_optimization();
            } else {
                controller.disable_performance_optimization();
            }
        }
    }

    fn read_actuation_point(&mut self) -> Result<u8> {
        if let Some(value) = self.actuation_point_cache {
            Ok(value)
        } else {
            Err(Error::DeviceCommunication(
                "Reading actuation point not yet implemented - HID read command not discovered and no cached value available. Hint: the actuation point can currently only be retrieved if it was set earlier in this session; cache the value you set instead of relying on reading it back.".to_string(),
            ))
        }
    }

    fn set_actuation_point(&mut self, _value: u8) -> Result<()> {
        Err(Error::DeviceCommunication(
            "Setting actuation point not supported on this keyboard model".to_string(),
        ))
    }

    fn set_actuation_point_mm(&mut self, _mm: f32) -> Result<()> {
        Err(Error::DeviceCommunication(
            "Setting actuation point not supported on this keyboard model".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::product_ids;
    use crate::devices::{HidCommand, PerKeyAddressingMode};

    fn create_test_device_info() -> DeviceInfo {
        DeviceInfo {
            name: "Test Apex Pro TKL 2023".to_string(),
            device_type: crate::devices::DeviceType::Keyboard,
            vendor_id: crate::STEELSERIES_VENDOR_ID,
            product_id: product_ids::APEX_PRO_TKL_2023,
            interface_number: 1,
            serial_number: Some("TEST123".to_string()),
            manufacturer: Some("SteelSeries".to_string()),
            path: "/test/device/path".to_string(),
        }
    }

    #[test]
    fn test_per_key_rgb_support_detection() {
        let _info = create_test_device_info();

        let db = KeyMappingDatabase::new();

        assert!(db.supports_product(product_ids::APEX_PRO_TKL_2023));
        assert!(db.get_mapping(product_ids::APEX_PRO_TKL_2023).is_some());

        assert!(!db.supports_product(0xFFFF));
        assert!(db.get_mapping(0xFFFF).is_none());
    }

    #[test]
    fn test_key_mapping_integration() {
        let db = KeyMappingDatabase::new();

        if let Some(mapping) = db.get_mapping(product_ids::APEX_PRO_TKL_2023) {
            assert!(mapping.supports_key(KeyId::A));
            assert!(mapping.supports_key(KeyId::Enter));
            assert!(mapping.supports_key(KeyId::Space));
            assert!(mapping.supports_key(KeyId::Escape));

            assert!(mapping.get_key_address(KeyId::A).is_some());
            assert!(mapping.get_key_address(KeyId::Enter).is_some());

            let stats = mapping.get_stats();
            assert!(stats.total_keys > 0);
            assert!(stats.utilization > 0.0);
        }
    }

    #[test]
    fn test_per_key_rgb_builder_integration() {
        let db = KeyMappingDatabase::new();

        if let Some(mapping) = db.get_mapping(product_ids::APEX_PRO_TKL_2023) {
            let mut builder = PerKeyRgbBuilder::with_key_mapping(mapping.clone());

            let result = builder.add_key_logical(KeyId::A, Color::RED);
            assert!(result.is_ok());

            let result = builder.add_key_logical(KeyId::S, Color::GREEN);
            assert!(result.is_ok());

            let result = builder.add_key_logical(KeyId::D, Color::BLUE);
            assert!(result.is_ok());

            assert_eq!(builder.key_count(), 3);

            let command = builder.build();
            assert_eq!(command.key_count(), 3);
            assert_eq!(command.addressing_mode, PerKeyAddressingMode::HidCode);

            assert!(command.validate().is_ok());
        }
    }

    #[test]
    fn test_zone_count_mapping() {
        assert_eq!(zone_count_for_product_id(product_ids::APEX_PRO_TKL_2023), 9);
        assert_eq!(zone_count_for_product_id(product_ids::APEX_3), 10);
        assert_eq!(zone_count_for_product_id(product_ids::APEX_3_TKL), 9);

        assert_eq!(zone_count_for_product_id(0xFFFF), 1);
    }

    #[test]
    #[allow(deprecated)]
    fn test_per_key_addressing_modes() {
        assert_ne!(PerKeyAddressingMode::Matrix, PerKeyAddressingMode::Logical);

        let matrix_builder = PerKeyRgbBuilder::new(PerKeyAddressingMode::Matrix);
        assert!(matrix_builder.is_empty());

        let db = KeyMappingDatabase::new();
        if let Some(mapping) = db.get_mapping(product_ids::APEX_PRO_TKL_2023) {
            let logical_builder = PerKeyRgbBuilder::with_key_mapping(mapping.clone());
            assert!(logical_builder.is_empty());
        }
    }

    #[test]
    fn test_key_address_bounds() {
        // KeyAddress now uses HID codes instead of row/col
        let addr = KeyAddress::new(30); // HID code 30 (1 key)
        assert_eq!(addr.hid_code, 30);

        let mut command = PerKeyRgbCommand::new(PerKeyAddressingMode::HidCode);

        command.set_key_color(KeyAddress::new(30), Color::RED);
        assert!(command.validate().is_ok());

        // HID code 0 is invalid (not a standard USB HID keycode)
        command.set_key_color(KeyAddress::new(0), Color::BLUE);
        assert!(command.validate().is_err());
    }

    #[test]
    fn test_supported_product_ids() {
        let db = KeyMappingDatabase::new();
        let supported_products = db.get_supported_products();

        assert!(supported_products.contains(&product_ids::APEX_PRO_TKL_2023));
        assert!(supported_products.contains(&product_ids::APEX_PRO));
        assert!(supported_products.contains(&product_ids::APEX_PRO_TKL));

        assert!(!supported_products.is_empty());
    }
}

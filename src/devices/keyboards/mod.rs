//! Keyboard device support (Apex series).

pub mod apex;
pub mod apex_pro_tkl_2023;

use super::diagnostics::{HidOperation, with_global_diagnostics};
use super::hid_reports::{
    ApplyCommand, BrightnessCommand, HidDeviceType, HidReportBuilder, PerKeyRgbBuilder,
    PerKeyRgbCommand, RgbZoneCommand,
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
    fn set_color(&mut self, color: Color) -> Result<()>;

    /// Set colors for individual zones.
    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()>;

    /// Get the number of RGB zones.
    fn zone_count(&self) -> usize;

    /// Set keyboard brightness (0-100).
    fn set_brightness(&mut self, brightness: u8) -> Result<()>;

    /// Apply the current RGB settings.
    fn apply(&mut self) -> Result<()>;

    // === Per-Key RGB Control ===

    /// Check if per-key RGB control is supported by this keyboard.
    fn supports_per_key_rgb(&self) -> bool;

    /// Get the key mapping for this keyboard (if available).
    fn get_key_mapping(&self) -> Option<&KeyMapping>;

    /// Set RGB color for a specific key by logical key ID.
    ///
    /// Uses the keyboard's key mapping to convert logical key IDs to matrix addresses.
    /// Returns an error if the key is not found in the mapping or per-key RGB is not supported.
    fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()>;

    /// Set RGB colors for multiple keys by logical key IDs.
    ///
    /// Uses the keyboard's key mapping to convert logical key IDs to matrix addresses.
    /// Keys not found in the mapping are ignored with a warning.
    fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()>;

    /// Set RGB color for a specific key by direct matrix address.
    ///
    /// Bypasses key mapping and directly addresses the key matrix.
    /// Use with caution - invalid addresses may cause device issues.
    fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()>;

    /// Set RGB colors for multiple keys by direct matrix addresses.
    ///
    /// Bypasses key mapping and directly addresses the key matrix.
    /// Use with caution - invalid addresses may cause device issues.
    fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()>;

    /// Set all keys to black (turn off per-key RGB).
    fn clear_per_key_rgb(&mut self) -> Result<()>;

    /// Set a region of keys to the same color using matrix coordinates.
    fn set_key_region(
        &mut self,
        start_row: u8,
        start_col: u8,
        rows: u8,
        cols: u8,
        color: Color,
    ) -> Result<()>;

    // === Zone-based RGB Fallback ===

    /// Get zone mapping information for this keyboard.
    fn get_zone_mapping(&self) -> Option<&ZoneMap>;

    /// Set zone-based RGB effect as fallback.
    async fn set_zone_effect(&mut self, effect: ZoneEffect) -> Result<()>;

    /// Simulate per-key effect using zone-based fallback.
    async fn simulate_per_key_with_zones(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()>;

    /// Enhanced zone-based RGB with retry logic.
    async fn set_zone_colors_with_retry(
        &mut self,
        colors: &[Color],
        max_retries: usize,
    ) -> Result<()>;

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
    fn apply_per_key_effect_with_brightness(&mut self, brightness: f32) -> Result<()>;

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
    #[allow(dead_code)]
    key_mapping_db: KeyMappingDatabase,
    zone_fallback: ZoneFallback,
    zone_mapping: Option<ZoneMap>,
    per_key_controller: Option<PerKeyRgbController>,
}

impl GenericKeyboard {
    /// Create a new keyboard instance.
    pub fn new(info: DeviceInfo, device: HidDevice) -> Self {
        // Use centralized zone count mapping
        let zone_count = zone_count_for_product_id(info.product_id);

        // Initialize key mapping database and try to find mapping for this keyboard
        let key_mapping_db = KeyMappingDatabase::new();
        let key_mapping = key_mapping_db.get_mapping(info.product_id).cloned();

        if key_mapping.is_some() {
            tracing::debug!(
                "Loaded key mapping for product ID 0x{:04x}",
                info.product_id
            );
        } else {
            tracing::warn!(
                "No key mapping available for product ID 0x{:04x} - per-key RGB disabled",
                info.product_id
            );
        }

        // Initialize zone fallback system
        let zone_fallback = ZoneFallback::new();
        let zone_mapping = zone_fallback.get_mapping(info.product_id).cloned();

        if zone_mapping.is_some() {
            tracing::debug!(
                "Loaded zone mapping for product ID 0x{:04x}",
                info.product_id
            );
        } else {
            tracing::warn!(
                "No zone mapping available for product ID 0x{:04x} - using basic zone fallback",
                info.product_id
            );
        }

        // Initialize per-key RGB controller if key mapping is available
        let per_key_controller = key_mapping.as_ref().map(|mapping| {
            tracing::debug!(
                "Initializing per-key RGB controller for product ID 0x{:04x}",
                info.product_id
            );
            // Use performance-optimized controller for better responsiveness
            PerKeyRgbController::new_with_performance(mapping.clone())
        });

        Self {
            info,
            device: Some(Arc::new(Mutex::new(device))),
            zone_count,
            report_builder: HidReportBuilder::new(HidDeviceType::Keyboard),
            key_mapping,
            key_mapping_db,
            zone_fallback,
            zone_mapping,
            per_key_controller,
        }
    }

    /// Send a HID report to the keyboard.
    fn send_report(&mut self, data: &[u8]) -> Result<()> {
        use tracing::debug;

        // Validate report structure if diagnostics enabled
        with_global_diagnostics(|diag| {
            if !diag.validate_report_structure(data) {
                debug!("HID report validation failed, sending anyway");
            }
        });

        debug!("Sending HID report ({} bytes): {:02x?}", data.len(), data);

        let device = self.device.as_ref().ok_or(Error::DeviceCommunication(
            "Device not connected".to_string(),
        ))?;
        let device = device.lock();

        // Record the operation with timing analysis
        let result = if let Some(result) = with_global_diagnostics(|diag| {
            diag.record_timed_operation(HidOperation::Send, data, || {
                write_padded_report(&device, data, 65, true)
            })
        }) {
            // Diagnostics handled the operation and returned result
            result
        } else {
            // No diagnostics, do normal operation
            write_padded_report(&device, data, 65, true)
        };

        match &result {
            Ok(_) => debug!("HID report sent successfully"),
            Err(e) => debug!("HID report failed: {:?}", e),
        }

        result
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
        // Send initialization sequence if needed
        // Some SteelSeries keyboards need a "save" or "commit" command
        let apply_command = ApplyCommand;
        if let Ok(data) = self.report_builder.build_report(apply_command) {
            let _ = self.send_report(&data); // Don't fail if this doesn't work
        }
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
        let device = self.device.as_ref().ok_or(Error::DeviceCommunication(
            "Device not connected".to_string(),
        ))?;
        let device = device.lock();

        // Record the operation with timing analysis
        if let Some(result) = with_global_diagnostics(|diag| {
            diag.record_timed_operation(HidOperation::Receive, &[], || {
                let len = device.read(buf)?;
                Ok((len, buf[..len].to_vec())) // Return both length and data for diagnostics
            })
        }) {
            result.map(|(len, _data)| len) // Extract just the length
        } else {
            // No diagnostics, do normal operation
            let len = device.read(buf)?;
            Ok(len)
        }
    }
}

impl GenericKeyboard {
    /// Helper method to compute average color from a list of key colors.
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
    fn set_color(&mut self, color: Color) -> Result<()> {
        // Create color data for all zones
        let colors = vec![color; self.zone_count];
        self.set_zone_colors(&colors)
    }

    fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        // Create structured RGB zone command
        let mut zone_colors = colors
            .iter()
            .take(self.zone_count)
            .copied()
            .collect::<Vec<_>>();

        // Pad remaining zones with black
        while zone_colors.len() < self.zone_count {
            zone_colors.push(Color::BLACK);
        }

        let rgb_command = RgbZoneCommand::new_all_zones(zone_colors);
        let data = self.report_builder.build_report(rgb_command)?;

        self.send_report(&data)
    }

    fn zone_count(&self) -> usize {
        self.zone_count
    }

    fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        // Create structured brightness command (auto-clamps to 0-100)
        let brightness_command = BrightnessCommand::new(brightness);
        let data = self.report_builder.build_report(brightness_command)?;

        self.send_report(&data)
    }

    fn apply(&mut self) -> Result<()> {
        // Create structured apply/save command
        let apply_command = ApplyCommand;
        let data = self.report_builder.build_report(apply_command)?;

        // Don't fail if device doesn't support apply command
        let _ = self.send_report(&data);
        Ok(())
    }

    // === Per-Key RGB Control Implementation ===

    fn supports_per_key_rgb(&self) -> bool {
        self.key_mapping.is_some()
    }

    fn get_key_mapping(&self) -> Option<&KeyMapping> {
        self.key_mapping.as_ref()
    }

    fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        if !self.supports_per_key_rgb() {
            return Err(Error::DeviceCommunication(
                "Per-key RGB not supported - no key mapping available".to_string(),
            ));
        }

        let mapping = self.key_mapping.as_ref().unwrap();
        if let Some(address) = mapping.get_key_address(key_id) {
            self.set_key_color_direct(address, color)
        } else {
            Err(Error::DeviceCommunication(format!(
                "Key {:?} not found in key mapping",
                key_id
            )))
        }
    }

    fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        if !self.supports_per_key_rgb() {
            return Err(Error::DeviceCommunication(
                "Per-key RGB not supported - no key mapping available".to_string(),
            ));
        }

        let mapping = self.key_mapping.as_ref().unwrap();
        let mut builder = PerKeyRgbBuilder::with_key_mapping(mapping.clone());

        for (key_id, color) in key_colors {
            if let Err(e) = builder.add_key_logical(*key_id, *color) {
                tracing::warn!("Failed to add key {:?}: {}", key_id, e);
                // Continue with other keys rather than failing entirely
            }
        }

        if builder.is_empty() {
            return Err(Error::DeviceCommunication(
                "No valid keys found in key mapping".to_string(),
            ));
        }

        let command = builder.build();
        let data = self.report_builder.build_report(command)?;
        self.send_report(&data)
    }

    fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()> {
        let command = PerKeyRgbCommand::single_key(address, color);
        let data = self.report_builder.build_report(command)?;
        self.send_report(&data)
    }

    fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        if key_colors.is_empty() {
            return Err(Error::DeviceCommunication(
                "No key colors provided".to_string(),
            ));
        }

        let mut builder = PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::Matrix);

        for (address, color) in key_colors {
            builder.add_key_matrix(*address, *color);
        }

        let command = builder.build();
        let data = self.report_builder.build_report(command)?;
        self.send_report(&data)
    }

    fn clear_per_key_rgb(&mut self) -> Result<()> {
        if let Some(ref mapping) = self.key_mapping {
            // Set all keys in the mapping to black
            let black_keys: Vec<(KeyId, Color)> = mapping
                .get_all_keys()
                .into_iter()
                .map(|key_id| (key_id, Color::BLACK))
                .collect();

            if !black_keys.is_empty() {
                self.set_key_colors(&black_keys)
            } else {
                Ok(()) // No keys to clear
            }
        } else {
            // No key mapping available, use matrix approach to clear common positions
            // This is a fallback that clears the most common key matrix positions
            let mut builder =
                PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::Matrix);

            // Clear typical keyboard matrix (6x17 for TKL, should cover most keys)
            for row in 0..6 {
                for col in 0..17 {
                    builder.add_key_matrix(KeyAddress::new(row, col), Color::BLACK);
                }
            }

            let command = builder.build();
            let data = self.report_builder.build_report(command)?;
            self.send_report(&data)
        }
    }

    fn set_key_region(
        &mut self,
        start_row: u8,
        start_col: u8,
        rows: u8,
        cols: u8,
        color: Color,
    ) -> Result<()> {
        let mut builder = PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::Matrix);
        builder.set_region(start_row, start_col, rows, cols, color);

        if builder.is_empty() {
            return Err(Error::DeviceCommunication(
                "Invalid region - no keys to set".to_string(),
            ));
        }

        let command = builder.build();
        let data = self.report_builder.build_report(command)?;
        self.send_report(&data)
    }

    // === Zone-based RGB Fallback Implementation ===

    fn get_zone_mapping(&self) -> Option<&ZoneMap> {
        self.zone_mapping.as_ref()
    }

    async fn set_zone_effect(&mut self, effect: ZoneEffect) -> Result<()> {
        // Set the effect in the fallback system for time-based effects
        self.zone_fallback.set_current_effect(effect.clone());

        // Apply the effect immediately
        let colors = effect.compute_colors(self.zone_count, 0.0);
        self.set_zone_colors_with_retry(&colors, 3).await
    }

    async fn simulate_per_key_with_zones(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        // Use the zone fallback system to convert per-key colors to zone colors
        if let Some(zone_colors) = self
            .zone_fallback
            .simulate_per_key_effect(self.info.product_id, key_colors)
        {
            self.set_zone_colors_with_retry(&zone_colors, 3).await
        } else {
            // Fallback to single color if no zone mapping available
            if !key_colors.is_empty() {
                let avg_color = self.compute_average_color(key_colors);
                self.set_color(avg_color)?;
                self.apply()
            } else {
                self.set_color(Color::BLACK)?;
                self.apply()
            }
        }
    }

    async fn set_zone_colors_with_retry(
        &mut self,
        colors: &[Color],
        max_retries: usize,
    ) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.set_zone_colors(colors) {
                Ok(()) => {
                    if attempt > 0 {
                        tracing::info!("Zone RGB succeeded on attempt {}", attempt + 1);
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries - 1 {
                        tracing::warn!(
                            "Zone RGB attempt {} failed, retrying: {:?}",
                            attempt + 1,
                            last_error
                        );
                        // Small delay before retry
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        }

        // All retries failed
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

        // Test each zone individually
        for zone_index in 0..self.zone_count {
            let mut zone_colors = vec![Color::BLACK; self.zone_count];
            zone_colors[zone_index] = Color::WHITE;

            let success = self.set_zone_colors(&zone_colors).is_ok();
            results.push(success);

            if !success {
                tracing::warn!("Zone {} failed reliability test", zone_index);
            }

            // Small delay between tests
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // Reset to all black after testing
        let _ = self.set_color(Color::BLACK);

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

            // Apply the effect immediately by getting colors and sending to device
            Some(controller.compute_key_colors().to_vec())
        } else {
            None
        };

        if let Some(colors) = key_colors {
            self.set_key_colors(&colors)?;
            self.apply()
        } else {
            // Fallback: convert to zone-based effect if no per-key support
            self.convert_per_key_to_zones(&effect).await
        }
    }

    fn get_per_key_effect(&self) -> Option<&PerKeyEffect> {
        self.per_key_controller.as_ref().map(|c| c.effect())
    }

    async fn trigger_key_reactive(&mut self, keys: &[KeyId], duration: f32) -> Result<()> {
        let key_colors = if let Some(ref mut controller) = self.per_key_controller {
            controller.trigger_reactive(keys, duration);

            // Apply the updated reactive state
            Some(controller.compute_key_colors().to_vec())
        } else {
            None
        };

        if let Some(colors) = key_colors {
            self.set_key_colors(&colors)?;
            self.apply()
        } else {
            // Fallback: simulate reactive effect using zones
            if !keys.is_empty() {
                // Use zone fallback to simulate reactive effect
                self.simulate_per_key_with_zones(
                    &keys.iter().map(|&k| (k, Color::WHITE)).collect::<Vec<_>>(),
                )
                .await
            } else {
                Ok(())
            }
        }
    }

    async fn apply_per_key_effect_with_brightness(&mut self, brightness: f32) -> Result<()> {
        let key_colors = if let Some(ref mut controller) = self.per_key_controller {
            controller.set_brightness(brightness.clamp(0.0, 1.0));

            // Apply the effect with new brightness
            Some(controller.compute_key_colors().to_vec())
        } else {
            None
        };

        if let Some(colors) = key_colors {
            self.set_key_colors(&colors)?;
            self.apply()
        } else {
            // Fallback: apply brightness to zones
            self.set_brightness((brightness * 100.0) as u8)?;
            self.apply()
        }
    }

    async fn convert_per_key_to_zones(&mut self, effect: &PerKeyEffect) -> Result<()> {
        // Convert per-key effect to zone-based effect
        let zone_effect = match effect {
            PerKeyEffect::Static { color } => ZoneEffect::Solid(*color),

            PerKeyEffect::Breathing { color, speed: _ } => {
                // Use solid color for zone-based breathing (animation handled by zone system)
                ZoneEffect::Breathing {
                    colors: vec![*color],
                    phase_offset: 0.0,
                }
            }

            PerKeyEffect::Spectrum { speed: _ } => {
                // Create rainbow effect across zones
                ZoneEffect::Wave {
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
                }
            }

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
            } => {
                // Use alternating colors to approximate gaming zones
                ZoneEffect::Alternating(vec![*wasd_color, *default_color])
            }

            PerKeyEffect::Custom { key_colors } => {
                // Average all custom colors for zone effect
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

            // For complex effects, use a reasonable fallback
            _ => ZoneEffect::Solid(Color::WHITE),
        };

        self.set_zone_effect(zone_effect).await
    }

    // === Performance Optimization Implementation ===

    fn get_rgb_performance_stats(&self) -> Option<&crate::performance::PerformanceStats> {
        self.per_key_controller
            .as_ref()
            .and_then(|c| c.get_performance_stats())
    }

    fn get_optimal_frame_time(&self) -> Option<std::time::Duration> {
        self.per_key_controller
            .as_ref()
            .and_then(|c| c.get_frame_time())
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
        // PLACEHOLDER: HID command to read actuation point not yet discovered
        //
        // When the read command is discovered, implementation should:
        // 1. Send query command (e.g., [0xXX] where XX is the read command byte)
        // 2. Call self.receive_raw() to read response
        // 3. Parse response to extract actuation value
        // 4. Return value in 0.1mm units (4 = 0.4mm, 36 = 3.6mm)
        //
        // For now, return DeviceCommunication error
        Err(Error::DeviceCommunication(
            "Reading actuation point not yet implemented - HID read command not discovered"
                .to_string(),
        ))
    }

    fn set_actuation_point(&mut self, _value: u8) -> Result<()> {
        // Placeholder implementation - most keyboards don't support this feature
        Err(Error::DeviceCommunication(
            "Setting actuation point not supported on this keyboard model".to_string(),
        ))
    }

    fn set_actuation_point_mm(&mut self, _mm: f32) -> Result<()> {
        // Placeholder implementation - most keyboards don't support this feature
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
        // Note: This test uses a mock HidDevice, but the keyboard creation process
        // will work with the key mapping database to determine per-key RGB support

        // For Apex Pro TKL 2023 (should have key mapping)
        let _info = create_test_device_info();

        // We can't actually create a GenericKeyboard without a real HidDevice
        // but we can test the key mapping database directly
        let db = KeyMappingDatabase::new();

        assert!(db.supports_product(product_ids::APEX_PRO_TKL_2023));
        assert!(db.get_mapping(product_ids::APEX_PRO_TKL_2023).is_some());

        // Test unknown product ID
        assert!(!db.supports_product(0xFFFF));
        assert!(db.get_mapping(0xFFFF).is_none());
    }

    #[test]
    fn test_key_mapping_integration() {
        let db = KeyMappingDatabase::new();

        if let Some(mapping) = db.get_mapping(product_ids::APEX_PRO_TKL_2023) {
            // Test that basic keys are mapped
            assert!(mapping.supports_key(KeyId::A));
            assert!(mapping.supports_key(KeyId::Enter));
            assert!(mapping.supports_key(KeyId::Space));
            assert!(mapping.supports_key(KeyId::Escape));

            // Test key address retrieval
            assert!(mapping.get_key_address(KeyId::A).is_some());
            assert!(mapping.get_key_address(KeyId::Enter).is_some());

            // Test mapping statistics
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

            // Test adding logical keys
            let result = builder.add_key_logical(KeyId::A, Color::RED);
            assert!(result.is_ok());

            let result = builder.add_key_logical(KeyId::S, Color::GREEN);
            assert!(result.is_ok());

            let result = builder.add_key_logical(KeyId::D, Color::BLUE);
            assert!(result.is_ok());

            assert_eq!(builder.key_count(), 3);

            // Build command
            let command = builder.build();
            assert_eq!(command.key_count(), 3);
            assert_eq!(command.addressing_mode, PerKeyAddressingMode::Logical);

            // Test validation
            assert!(command.validate().is_ok());
        }
    }

    #[test]
    fn test_zone_count_mapping() {
        // Test zone count for known keyboards
        assert_eq!(zone_count_for_product_id(product_ids::APEX_PRO_TKL_2023), 9);
        assert_eq!(zone_count_for_product_id(product_ids::APEX_3), 10);
        assert_eq!(zone_count_for_product_id(product_ids::APEX_3_TKL), 9);

        // Test unknown product ID (should default to 1)
        assert_eq!(zone_count_for_product_id(0xFFFF), 1);
    }

    #[test]
    fn test_per_key_addressing_modes() {
        // Test that addressing modes are distinct
        assert_ne!(PerKeyAddressingMode::Matrix, PerKeyAddressingMode::Logical);

        // Test builder creation with different modes
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
        // Test key address creation and bounds
        let addr = KeyAddress::new(5, 16);
        assert_eq!(addr.row, 5);
        assert_eq!(addr.col, 16);

        // Test command validation with bounds
        let mut command = PerKeyRgbCommand::new(PerKeyAddressingMode::Matrix);

        // Valid address should work
        command.set_key_color(KeyAddress::new(5, 16), Color::RED);
        assert!(command.validate().is_ok());

        // Invalid address should fail validation
        command.set_key_color(KeyAddress::new(32, 0), Color::BLUE);
        assert!(command.validate().is_err());
    }

    #[test]
    fn test_supported_product_ids() {
        let db = KeyMappingDatabase::new();
        let supported_products = db.get_supported_products();

        // Should include known Apex Pro models
        assert!(supported_products.contains(&product_ids::APEX_PRO_TKL_2023));
        assert!(supported_products.contains(&product_ids::APEX_PRO));
        assert!(supported_products.contains(&product_ids::APEX_PRO_TKL));

        // Should not be empty
        assert!(!supported_products.is_empty());
    }
}

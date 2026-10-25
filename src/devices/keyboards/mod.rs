//! Keyboard device support (Apex series).

pub mod apex;
pub mod apex_7;
pub mod apex_7_profile;
pub mod apex_m750;
pub mod apex_pro_tkl_2023;
pub mod apex_pro_tkl_2023_profile;
pub mod onboard;

use super::diagnostics::{HidOperation, with_global_diagnostics};
use super::hid_reports::{
    BrightnessCommand, HidDeviceType, HidReportBuilder, PerKeyRgbBuilder, PerKeyRgbCommand, RgbZoneCommand,
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
    async fn set_color(&mut self, _color: Color) -> Result<()> {
        Err(Error::DeviceCommunication(
            "set_color is not supported for this keyboard".to_string(),
        ))
    }

    /// Set colors for individual zones.
    async fn set_zone_colors(&mut self, _colors: &[Color]) -> Result<()> {
        Ok(())
    }

    /// Get the number of RGB zones.
    fn zone_count(&self) -> usize {
        0
    }

    /// Set keyboard brightness (0-100).
    async fn set_brightness(&mut self, _brightness: u8) -> Result<()> {
        Ok(())
    }

    /// Apply the current RGB settings.
    async fn apply(&mut self) -> Result<()> {
        Ok(())
    }

    // === Per-Key RGB Control ===

    /// Check if per-key RGB key-mapping is available for this keyboard.
    ///
    /// ⚠️ **Experimental**: returning `true` means a HID key mapping exists, but the
    /// per-key protocol (`CommandCode::PerKeyRgb`, `0x23`) has not been confirmed on
    /// hardware. Calls to `set_key_color` / `set_key_colors` may be silently ignored by
    /// the device until the packet format is verified via USB capture.
    fn supports_per_key_rgb(&self) -> bool {
        false
    }

    /// Get the key mapping for this keyboard (if available).
    fn get_key_mapping(&self) -> Option<&KeyMapping> {
        None
    }

    /// Set RGB color for a specific key by logical key ID.
    async fn set_key_color(&mut self, _key_id: KeyId, _color: Color) -> Result<()> {
        Err(Error::DeviceCommunication("Per-key RGB not supported".to_string()))
    }

    /// Set RGB colors for multiple keys by logical key IDs.
    async fn set_key_colors(&mut self, _key_colors: &[(KeyId, Color)]) -> Result<()> {
        Ok(())
    }

    /// Set RGB color for a specific key by direct HID code address.
    async fn set_key_color_direct(&mut self, _address: KeyAddress, _color: Color) -> Result<()> {
        Ok(())
    }

    /// Set RGB colors for multiple keys by direct HID code addresses.
    async fn set_key_colors_direct(&mut self, _key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        Ok(())
    }

    /// Set all keys to black (turn off per-key RGB).
    async fn clear_per_key_rgb(&mut self) -> Result<()> {
        Ok(())
    }

    /// Set a region of keys to the same color using HID codes.
    async fn set_key_region(&mut self, _start_hid: u8, _count: u8, _color: Color) -> Result<()> {
        Ok(())
    }

    // === Zone-based RGB Fallback ===

    /// Get zone mapping information for this keyboard.
    fn get_zone_mapping(&self) -> Option<&ZoneMap> {
        None
    }

    /// Set zone-based RGB effect as fallback.
    async fn set_zone_effect(&mut self, _effect: ZoneEffect) -> Result<()> {
        Ok(())
    }

    /// Simulate per-key effect using zone-based fallback.
    async fn simulate_per_key_with_zones(&mut self, _key_colors: &[(KeyId, Color)]) -> Result<()> {
        Ok(())
    }

    /// Enhanced zone-based RGB with retry logic.
    async fn set_zone_colors_with_retry(&mut self, _colors: &[Color], _max_retries: usize) -> Result<()> {
        Ok(())
    }

    /// Test zone connectivity and reliability.
    async fn test_zone_reliability(&mut self) -> Result<Vec<bool>> {
        Err(Error::from(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "test_zone_reliability is not supported for this keyboard",
        )))
    }

    // === Per-Key RGB Effect Support ===

    /// Check if per-key RGB effects are supported.
    fn supports_per_key_effects(&self) -> bool {
        false
    }

    /// Set per-key RGB effect.
    async fn set_per_key_effect(&mut self, _effect: PerKeyEffect) -> Result<()> {
        Ok(())
    }

    /// Get current per-key RGB effect (if available).
    fn get_per_key_effect(&self) -> Option<&PerKeyEffect> {
        None
    }

    /// Trigger reactive effect for specific keys.
    async fn trigger_key_reactive(&mut self, _keys: &[KeyId], _duration: f32) -> Result<()> {
        Ok(())
    }

    /// Apply per-key effect with brightness control.
    async fn apply_per_key_effect_with_brightness(&mut self, _brightness: f32) -> Result<()> {
        Ok(())
    }

    /// Convert per-key effect to zone-based fallback.
    async fn convert_per_key_to_zones(&mut self, _effect: &PerKeyEffect) -> Result<()> {
        Ok(())
    }

    // === Performance Optimization ===

    /// Get current performance statistics for RGB operations.
    fn get_rgb_performance_stats(&self) -> Option<&crate::performance::PerformanceStats> {
        None
    }

    /// Get optimal frame time for current adaptive refresh rate.
    fn get_optimal_frame_time(&self) -> Option<std::time::Duration> {
        None
    }

    /// Force cleanup of performance caches.
    fn cleanup_rgb_caches(&mut self) {}

    /// Enable/disable performance optimizations.
    fn set_performance_optimization(&mut self, _enabled: bool) {}

    /// Read current actuation point setting from keyboard (if supported).
    ///
    /// **NOTE**: This is currently a placeholder. The HID command to read
    /// actuation settings has not yet been discovered. This function always
    /// returns an error indicating the feature is not implemented.
    ///
    /// Returns actuation point in 0.1mm units (e.g., 4 = 0.4mm, 36 = 3.6mm).
    fn read_actuation_point(&mut self) -> Result<u8> {
        Err(Error::DeviceCommunication(
            "Reading actuation point not implemented".to_string(),
        ))
    }

    /// Set actuation point for adjustable actuation keyboards (if supported).
    ///
    /// Value is in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm).
    /// Returns an error if the keyboard doesn't support adjustable actuation.
    fn set_actuation_point(&mut self, _value: u8) -> Result<()> {
        Err(Error::DeviceCommunication(
            "Setting actuation point not supported".to_string(),
        ))
    }

    /// Set actuation point for adjustable actuation keyboards in millimeters (if supported).
    ///
    /// Value is in millimeters with precision limited to 0.1mm increments.
    /// Returns an error if the keyboard doesn't support adjustable actuation.
    fn set_actuation_point_mm(&mut self, _mm: f32) -> Result<()> {
        Err(Error::DeviceCommunication(
            "Setting actuation point not supported".to_string(),
        ))
    }

    /// [EXPERIMENTAL] Send a live analog-key setting (actuation, release mode, rapid trigger, rapid tap).
    /// Only the wired Apex Pro TKL 2023 (`0x1628`) defines these; other keyboards return an error. Returns the
    /// board's answer, if any.
    fn send_analog_setting(&mut self, _setting: &apex_pro_tkl_2023::AnalogSetting) -> Result<Option<Vec<u8>>> {
        Err(Error::DeviceCommunication(
            "live analog-key settings are not supported by this keyboard".to_string(),
        ))
    }

    /// [EXPERIMENTAL] Send a 2022-firmware display command (leave direct mode, OLED). Wired Apex Pro TKL 2023
    /// only; other keyboards return an error.
    fn send_display_command(
        &mut self,
        _command: crate::devices::hid_reports::Apex2022DisplayCommand<'_>,
    ) -> Result<()> {
        Err(Error::DeviceCommunication(
            "this display command is not supported by this keyboard".to_string(),
        ))
    }

    /// [EXPERIMENTAL] Read-only Gen3 on-board profile read of one slot (0-4). Wired Apex Pro TKL 2023 only;
    /// other keyboards return an error.
    fn read_gen3_profile(&mut self, _slot: u8) -> Result<apex_pro_tkl_2023::Gen3ProfileRead> {
        Err(Error::DeviceCommunication(
            "the Gen3 on-board profile read is not supported by this keyboard".to_string(),
        ))
    }

    // === On-board profiles ===

    /// The on-board profile protocol this keyboard speaks, as the family codec that can read and write
    /// its bytes. `None` (the default) says this keyboard has no on-board profile protocol.
    ///
    /// A codec is a family-wide, device-free value: it decodes, encodes and reports payloads, and names
    /// the slots. A keyboard that carries a protocol overrides this with its family's codec, which
    /// [`onboard::codecs`](crate::devices::keyboards::onboard::codecs) also lists for file-only commands.
    fn onboard_codec(&self) -> Option<&'static dyn crate::devices::keyboards::onboard::OnboardProfileCodec> {
        None
    }

    /// Whether this keyboard answers the on-board profile protocol: whether [`Self::onboard_codec`] answers.
    fn supports_onboard_profile(&self) -> bool {
        self.onboard_codec().is_some()
    }

    /// Read one on-board profile slot (0-based; [`OnboardProfileCodec::slot_for_config`](crate::devices::keyboards::onboard::OnboardProfileCodec::slot_for_config)
    /// is what maps GG's config number to it). Keyboards that do not carry the protocol return an error.
    async fn read_onboard_profile(&self, _profile: u8) -> Result<crate::devices::keyboards::onboard::ProfileImage> {
        Err(Error::DeviceCommunication(
            "on-board profile read is not supported by this keyboard".to_string(),
        ))
    }

    /// Write one on-board profile slot (0-based) with `payload` bytes: the frames come
    /// from the keyboard's own codec ([`OnboardProfileCodec::write_plan`]), and the keyboard ends the write
    /// with the profile deploy frame (0x89). **This changes the keyboard's stored profile and makes it run
    /// the slot written.** Keyboards without a known write sequence return an error.
    async fn write_onboard_profile(
        &self,
        _profile: u8,
        _payload: &[u8],
    ) -> Result<crate::devices::keyboards::onboard::WriteStats> {
        Err(Error::DeviceCommunication(
            "on-board profile write is not supported by this keyboard".to_string(),
        ))
    }

    /// Make the keyboard run one on-board profile slot (0-based): GG's profile deploy frame (0x89), the
    /// `1-based` profile id on the wire. **This changes what the keyboard is running right now.** Keyboards
    /// that do not answer the frame return an error.
    async fn load_onboard_profile(&self, _profile: u8) -> Result<()> {
        Err(Error::DeviceCommunication(
            "on-board profile deploy is not supported by this keyboard".to_string(),
        ))
    }
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
        let per_key_controller = key_mapping
            .as_ref()
            .map(|mapping| PerKeyRgbController::new_with_performance(mapping.clone()));

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

    // Packet I/O on the collection this keyboard already owns.
    pub(crate) fn connected_device(&self) -> Result<&Arc<Mutex<HidDevice>>> {
        self.device
            .as_ref()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))
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

        let device = self.connected_device()?.lock();

        // `data` is builder output, report ID first; prepending another shifted every command by one byte.
        let result = if let Some(result) = with_global_diagnostics(|diag| {
            diag.record_timed_operation(HidOperation::Send, data, || {
                write_padded_report(&device, data, 65, false)
            })
        }) {
            result
        } else {
            write_padded_report(&device, data, 65, false)
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
    /// Build and send a HID command report to the keyboard asynchronously.
    async fn send_command_async<C: crate::devices::HidCommand>(&self, command: C) -> Result<()> {
        let mut buffer = [0u8; crate::devices::KEYBOARD_REPORT_SIZE];
        let size = self.report_builder.build_report(command, &mut buffer)?;
        self.send_report_async(&buffer[..size]).await
    }

    async fn send_report_async(&self, data: &[u8]) -> Result<()> {
        use tracing::debug;

        with_global_diagnostics(|diag| {
            if !diag.validate_report_structure(data) {
                debug!("HID report validation failed, sending anyway");
            }
        });

        debug!("Sending HID report ({} bytes): {:02x?}", data.len(), data);

        let device = self.connected_device()?.clone();

        let data = data.to_vec();

        let result = tokio::task::spawn_blocking(move || {
            let device = device.lock();

            if let Some(result) = with_global_diagnostics(|diag| {
                diag.record_timed_operation(HidOperation::Send, &data, || {
                    write_padded_report(&device, &data, 65, false)
                })
            }) {
                result
            } else {
                write_padded_report(&device, &data, 65, false)
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

    /// Send a HID feature report of arbitrary size (the Apex 2023 direct-mode packets): hidapi's
    /// `send_feature_report`, i.e. the kernel's `HIDIOCSFEATURE` on Linux, on the handle the device
    /// manager opened.
    ///
    /// `[EXPERIMENTAL]` - **needs a hardware test on the wired Apex Pro TKL 2023 (`0x1628`)**: if that
    /// board ignores frames sent on the collection discovery opened, its interface-1 control collection has
    /// to be opened as a device of its own.
    #[cfg(unix)]
    pub fn send_feature(&self, data: &[u8], report_len: usize) -> Result<()> {
        let device = self.connected_device()?;
        let report = {
            let mut report = vec![0u8; report_len.max(1)];
            let copy_len = data.len().min(report.len());
            report[..copy_len].copy_from_slice(&data[..copy_len]);
            report
        };
        let device = device.lock();
        device
            .send_feature_report(&report)
            .map_err(|e| Error::DeviceCommunication(e.to_string()))
    }

    #[cfg(not(unix))]
    pub fn send_feature(&self, data: &[u8], _report_len: usize) -> Result<()> {
        let device = self.connected_device()?.lock();

        device
            .send_feature_report(data)
            .map_err(|e| Error::DeviceCommunication(e.to_string()))
    }

    async fn send_zone_buffer_async(&mut self) -> Result<()> {
        // Wireless keyboards (e.g. Apex Pro TKL 2023 Wireless, PID 0x1632) don't
        // support the 0xFF "all zones" selector. Send per-zone commands instead,
        // with zone indices starting at 1.
        if self.info.product_id == super::product_ids::APEX_PRO_TKL_2023_WIRELESS
            || self.info.product_id == super::product_ids::APEX_PRO_TKL_2023_WIRELESS_2
        {
            for i in 0..self.zone_color_buffer.len() {
                let color = self.zone_color_buffer[i];
                let zone_index = (i + 1) as u8; // zones start at 1, not 0
                self.send_command_async(RgbZoneCommand::new_specific_zone(zone_index, color))
                    .await?;
            }
            return Ok(());
        }

        self.send_command_async(RgbZoneCommand::new_all_zones(&self.zone_color_buffer))
            .await
    }

    /// Send one output report on this keyboard's collection (`SET_REPORT wValue 0x0200`).
    pub(crate) fn write_profile_packet(&self, data: &[u8]) -> Result<()> {
        let device = self.connected_device()?.lock();
        let written = device
            .write(data)
            .map_err(|e| Error::DeviceCommunication(format!("output report write: {e}")))?;
        if written != data.len() {
            return Err(Error::DeviceCommunication(format!(
                "output report write accepted {written} of {} bytes",
                data.len()
            )));
        }
        Ok(())
    }

    /// Read one feature report from this keyboard's collection (`GET_REPORT wValue 0x0300`).
    pub(crate) fn read_profile_feature_packet(&self, report_id: u8, len: usize) -> Result<Vec<u8>> {
        if len < 2 {
            return Err(Error::DeviceCommunication(format!(
                "feature report size {len} cannot hold a report ID byte and any data"
            )));
        }
        let mut report = vec![report_id; len];
        let device = self.connected_device()?.lock();
        let received = device
            .get_feature_report(&mut report)
            .map_err(|e| Error::DeviceCommunication(format!("feature report read: {e}")))?;
        report.truncate(received.min(len));
        Ok(report)
    }

    /// Read one interrupt-endpoint input report from this keyboard's collection, waiting up to
    /// `timeout_ms`.
    pub(crate) fn read_profile_input_packet(&self, len: usize, timeout_ms: u64) -> Result<Option<Vec<u8>>> {
        let mut report = vec![0u8; len.max(1)];
        let poll_ms = i32::try_from(timeout_ms).unwrap_or(i32::MAX);
        let received = {
            let device = self.connected_device()?.lock();
            device
                .read_timeout(&mut report, poll_ms)
                .map_err(|e| Error::DeviceCommunication(format!("input report read: {e}")))?
        };
        if received == 0 {
            return Ok(None);
        }
        report.truncate(received);
        Ok(Some(report))
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

    // No APPLY (`00 09`): GG's specs name it `save_to_flash` on several Apex families, a persistent write, and
    // before the report-ID fix (2026-10-25) it only ever reached boards shifted, as an unknown `00` command.
    fn initialize(&mut self) -> Result<()> {
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
        let device = self.connected_device()?.lock();

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

        let (total_r, total_g, total_b) = key_colors.iter().fold((0u32, 0u32, 0u32), |(r, g, b), (_, color)| {
            (r + color.r as u32, g + color.g as u32, b + color.b as u32)
        });

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
        self.send_command_async(BrightnessCommand::new(brightness)).await
    }

    // No APPLY, for the reason on `initialize`.
    async fn apply(&mut self) -> Result<()> {
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
        for fragment in command.fragment_into_reports() {
            self.send_command_async(fragment).await?;
        }
        Ok(())
    }

    async fn set_key_color_direct(&mut self, address: KeyAddress, color: Color) -> Result<()> {
        self.send_command_async(PerKeyRgbCommand::single_key(address, color))
            .await
    }

    async fn set_key_colors_direct(&mut self, key_colors: &[(KeyAddress, Color)]) -> Result<()> {
        if key_colors.is_empty() {
            return Err(Error::DeviceCommunication("No key colors provided".to_string()));
        }

        let mut builder = PerKeyRgbBuilder::new(super::hid_reports::PerKeyAddressingMode::HidCode);

        for (address, color) in key_colors {
            builder.add_key_matrix(*address, *color);
        }

        self.send_command_async(builder.build()).await
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

        self.send_command_async(builder.build()).await
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
                colors: Color::RAINBOW_COLORS.to_vec(),
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
                    let (total_r, total_g, total_b) =
                        key_colors.values().fold((0u32, 0u32, 0u32), |(r, g, b), color| {
                            (r + color.r as u32, g + color.g as u32, b + color.b as u32)
                        });
                    let count = key_colors.len() as u32;

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
}

/// Macro to delegate common `Keyboard` trait methods to `self.inner`.
/// This avoids duplicating dozens of identical trait method implementations.
///
/// `apply` is deliberately not generated here: devices that stream colours in
/// direct mode must suppress it, so each implementation states its own policy.
#[macro_export]
macro_rules! impl_keyboard_with_delegation {
    ($type:ty, { $($custom:item)* }) => {
        #[async_trait]
        impl Keyboard for $type {
            $($custom)*
            fn zone_count(&self) -> usize { self.inner.zone_count() }
            fn supports_per_key_rgb(&self) -> bool { self.inner.supports_per_key_rgb() }
            fn get_key_mapping(&self) -> Option<&KeyMapping> { self.inner.get_key_mapping() }
            async fn clear_per_key_rgb(&mut self) -> Result<()> { self.set_color(Color::BLACK).await }
            fn get_zone_mapping(&self) -> Option<&ZoneMapping> { self.inner.get_zone_mapping() }
            async fn set_zone_effect(&mut self, effect: ZoneEffect) -> Result<()> { self.inner.set_zone_effect(effect).await }
            async fn simulate_per_key_with_zones(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> { self.inner.simulate_per_key_with_zones(key_colors).await }
            async fn set_zone_colors_with_retry(&mut self, colors: &[Color], max_retries: usize) -> Result<()> { self.inner.set_zone_colors_with_retry(colors, max_retries).await }
            async fn test_zone_reliability(&mut self) -> Result<Vec<bool>> { self.inner.test_zone_reliability().await }
            fn supports_per_key_effects(&self) -> bool { self.inner.supports_per_key_effects() }
            async fn set_per_key_effect(&mut self, effect: PerKeyEffect) -> Result<()> { self.inner.set_per_key_effect(effect).await }
            fn get_per_key_effect(&self) -> Option<&PerKeyEffect> { self.inner.get_per_key_effect() }
            async fn trigger_key_reactive(&mut self, keys: &[KeyId], duration: f32) -> Result<()> { self.inner.trigger_key_reactive(keys, duration).await }
            async fn apply_per_key_effect_with_brightness(&mut self, brightness: f32) -> Result<()> { self.inner.apply_per_key_effect_with_brightness(brightness).await }
            async fn convert_per_key_to_zones(&mut self, effect: &PerKeyEffect) -> Result<()> { self.inner.convert_per_key_to_zones(effect).await }
            fn get_rgb_performance_stats(&self) -> Option<&$crate::performance::PerformanceStats> { self.inner.get_rgb_performance_stats() }
            fn get_optimal_frame_time(&self) -> Option<std::time::Duration> { self.inner.get_optimal_frame_time() }
            fn cleanup_rgb_caches(&mut self) { self.inner.cleanup_rgb_caches() }
            fn set_performance_optimization(&mut self, enabled: bool) { self.inner.set_performance_optimization(enabled) }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::product_ids;
    use crate::devices::{HidCommand, PerKeyAddressingMode};

    fn create_test_device_info() -> DeviceInfo {
        DeviceInfo {
            name: std::borrow::Cow::Borrowed("Test Apex Pro TKL 2023"),
            device_type: crate::devices::DeviceType::Keyboard,
            vendor_id: crate::STEELSERIES_VENDOR_ID,
            product_id: product_ids::APEX_PRO_TKL_2023,
            interface_number: 1,
            usage_page: 0xFFC0,
            usage: 0x01,
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
        assert_eq!(zone_count_for_product_id(product_ids::APEX_3_TKL), 8);

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

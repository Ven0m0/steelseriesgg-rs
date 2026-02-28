    async fn send_report_async(&self, data: Vec<u8>) -> Result<()> {
        use tracing::debug;

        // Validate report structure if diagnostics enabled
        with_global_diagnostics(|diag| {
            if !diag.validate_report_structure(&data) {
                debug!("HID report validation failed, sending anyway");
            }
        });

        debug!("Sending HID report ({} bytes): {:02x?}", data.len(), data);

        let device = self
            .device
            .clone()
            .ok_or(Error::DeviceCommunication("Device not connected".to_string()))?;

        // Clone data for closure
        let data_clone = data.clone();

        let result = tokio::task::spawn_blocking(move || {
            let device = device.lock();

            // Record the operation with timing analysis
            if let Some(result) = with_global_diagnostics(|diag| {
                diag.record_timed_operation(HidOperation::Send, &data_clone, || {
                    write_padded_report(&device, &data_clone, 65, true)
                })
            }) {
                // Diagnostics handled the operation and returned result
                result
            } else {
                // No diagnostics, do normal operation
                write_padded_report(&device, &data_clone, 65, true)
            }
        })
        .await
        .map_err(|e| Error::DeviceCommunication(format!("Join error: {}", e)))?;

        match &result {
            Ok(_) => debug!("HID report sent successfully"),
            Err(e) => debug!("HID report failed: {:?}", e),
        }

        result
    }

    async fn send_zone_buffer_async(&mut self) -> Result<()> {
        let rgb_command = RgbZoneCommand::new_all_zones(&self.zone_color_buffer);
        let data = self.report_builder.build_report(rgb_command)?;
        self.send_report_async(data).await
    }

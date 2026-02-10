//! HID communication diagnostics and analysis tools.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::{Error, Result};

/// HID communication diagnostic data.
#[derive(Debug, Clone)]
pub struct HidDiagnostic {
    /// Timestamp of the operation
    pub timestamp: Instant,
    /// Operation type (send/receive)
    pub operation: HidOperation,
    /// Raw HID report data
    pub data: Vec<u8>,
    /// Operation duration
    pub duration: Duration,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// HID operation type.
#[derive(Debug, Clone)]
pub enum HidOperation {
    /// Sending data to device
    Send,
    /// Receiving data from device
    Receive,
    /// Device initialization
    Initialize,
    /// Device query/read
    Query,
}

impl std::fmt::Display for HidOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HidOperation::Send => write!(f, "SEND"),
            HidOperation::Receive => write!(f, "RECV"),
            HidOperation::Initialize => write!(f, "INIT"),
            HidOperation::Query => write!(f, "QUERY"),
        }
    }
}

/// HID diagnostic collector and analyzer.
pub struct HidDiagnostics {
    /// Whether diagnostics are enabled
    enabled: bool,
    /// Collected diagnostic entries
    diagnostics: Vec<HidDiagnostic>,
    /// Output file path for logging
    output_file: Option<PathBuf>,
    /// File handle for writing
    file_handle: Option<std::fs::File>,
}

impl HidDiagnostics {
    /// Create new diagnostics collector.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            diagnostics: Vec::new(),
            output_file: None,
            file_handle: None,
        }
    }

    /// Enable diagnostic logging to a timestamped file.
    pub fn enable_file_logging(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Create timestamped filename
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("ssgg_hid_diagnostics_{}.log", timestamp);
        let filepath = PathBuf::from(filename);

        // Open file for writing
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filepath)
            .map_err(Error::Io)?;

        debug!("HID diagnostics logging to: {:?}", filepath);

        self.output_file = Some(filepath);
        self.file_handle = Some(file);

        // Write header
        if let Some(ref mut file) = self.file_handle {
            writeln!(file, "# SteelSeries HID Communication Diagnostics")?;
            writeln!(file, "# Generated: {}", chrono::Utc::now())?;
            writeln!(
                file,
                "# Format: [timestamp] [operation] [duration_ms] [success] [size] [data_hex] [error]"
            )?;
            writeln!(file)?;
            file.flush()?;
        }

        Ok(())
    }

    /// Record a HID operation for diagnostic analysis.
    pub fn record_operation(&mut self, operation: HidOperation, data: &[u8], duration: Duration, result: &Result<()>) {
        if !self.enabled {
            return;
        }

        let diagnostic = HidDiagnostic {
            timestamp: Instant::now(),
            operation,
            data: data.to_vec(),
            duration,
            success: result.is_ok(),
            error: result.as_ref().err().map(|e| e.to_string()),
        };

        // Log to console
        let status = if diagnostic.success { "OK" } else { "FAIL" };
        let data_hex = diagnostic
            .data
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");

        debug!(
            "HID {} [{}] {:?} | {} bytes: {} | {:?}",
            diagnostic.operation,
            status,
            diagnostic.duration,
            diagnostic.data.len(),
            &data_hex[..data_hex.len().min(48)], // Truncate for readability
            diagnostic.error.as_deref().unwrap_or("Success")
        );

        // Log to file
        if let Some(ref mut file) = self.file_handle {
            let timestamp_str = diagnostic.timestamp.elapsed().as_millis();
            let error_str = diagnostic.error.as_deref().unwrap_or("");

            if let Err(e) = writeln!(
                file,
                "[{}] {} {:.2}ms {} {} {} {}",
                timestamp_str,
                diagnostic.operation,
                diagnostic.duration.as_secs_f64() * 1000.0,
                if diagnostic.success { "OK" } else { "FAIL" },
                diagnostic.data.len(),
                data_hex,
                error_str
            ) {
                warn!("Failed to write to diagnostic file: {}", e);
            } else if let Err(e) = file.flush() {
                warn!("Failed to flush diagnostic file: {}", e);
            }
        }

        self.diagnostics.push(diagnostic);
    }

    /// Record a timed HID operation.
    pub fn record_timed_operation<F, T>(&mut self, operation: HidOperation, data: &[u8], mut func: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let start_time = Instant::now();
        let result = func();
        let duration = start_time.elapsed();

        let simple_result: Result<()> = result.as_ref().map(|_| ()).map_err(|e| {
            // Convert the error to our Error type
            Error::Other(e.to_string())
        });
        self.record_operation(operation, data, duration, &simple_result);

        result
    }

    /// Validate HID report structure.
    pub fn validate_report_structure(&mut self, data: &[u8]) -> bool {
        if !self.enabled {
            return true; // Skip validation if diagnostics disabled
        }

        let mut valid = true;
        let mut issues = Vec::new();

        // Check report length
        if data.len() != 65 {
            issues.push(format!("Invalid report length: {} (expected 65)", data.len()));
            valid = false;
        }

        // Check report ID (first byte should typically be 0x00)
        if !data.is_empty() && data[0] != 0x00 {
            issues.push(format!("Unexpected report ID: 0x{:02x} (expected 0x00)", data[0]));
        }

        // Check for valid command bytes (known SteelSeries commands)
        if data.len() >= 2 {
            match data[1] {
                0x09 => { /* Apply/Save - valid */ }
                0x21 => { /* RGB control - valid */ }
                0x22 => { /* Brightness - valid */ }
                0x25 => { /* Reactive mode - valid */ }
                0x26 => { /* Color shift - valid */ }
                _ => {
                    issues.push(format!("Unknown command byte: 0x{:02x}", data[1]));
                }
            }
        }

        // Check RGB data structure for 0x21 commands
        if data.len() >= 3 && data[1] == 0x21 && data[2] == 0xFF {
            // RGB zone command - validate color data
            let color_data_start = 3;
            let max_zones = 9; // Apex Pro TKL 2023
            let expected_color_bytes = max_zones * 3; // 3 bytes per zone (RGB)

            if data.len() < color_data_start + expected_color_bytes {
                issues.push(format!(
                    "Insufficient RGB data: {} zones worth of data expected",
                    max_zones
                ));
            }
        }

        // Checksum validation
        if let Some(checksum_issue) = self.validate_checksum(data) {
            // Just log as info/debug for now as we are unsure of the exact algorithm
            debug!("Checksum analysis: {}", checksum_issue);
        }

        // Log validation results
        if !valid || !issues.is_empty() {
            let data_preview = data
                .iter()
                .take(16)
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");

            if valid {
                debug!("HID report validation warnings: {:?} | Data: {}", issues, data_preview);
            } else {
                warn!("HID report validation failed: {:?} | Data: {}", issues, data_preview);
            }
        }

        valid
    }

    /// Calculate simple Sum (mod 256) checksum of data excluding the last byte.
    fn calculate_checksum_sum(&self, data: &[u8]) -> u8 {
        if data.len() < 2 {
            return 0;
        }
        data.iter()
            .take(data.len() - 1)
            .fold(0u8, |acc, &x| acc.wrapping_add(x))
    }

    /// Calculate XOR checksum of data excluding the last byte.
    fn calculate_checksum_xor(&self, data: &[u8]) -> u8 {
        if data.len() < 2 {
            return 0;
        }
        data.iter().take(data.len() - 1).fold(0u8, |acc, &x| acc ^ x)
    }

    /// Validate checksum against common algorithms.
    /// Returns a message if a potential checksum match is found or if it looks like a checksum is missing.
    fn validate_checksum(&self, data: &[u8]) -> Option<String> {
        if data.len() < 2 {
            return None;
        }

        let last_byte = *data.last().unwrap();
        let sum = self.calculate_checksum_sum(data);
        let xor = self.calculate_checksum_xor(data);

        // Heuristic: If last byte matches a calculated checksum, it's interesting.
        if last_byte == sum {
            return Some(format!("Last byte (0x{:02x}) matches SUM checksum", last_byte));
        }
        if last_byte == xor {
            return Some(format!("Last byte (0x{:02x}) matches XOR checksum", last_byte));
        }

        // If data is long and last byte is 0, but sum/xor are non-zero, it might be padding instead of checksum
        if last_byte == 0 && (sum != 0 || xor != 0) {
            return Some("Last byte is 0x00 (likely padding), but non-zero checksums calculated".to_string());
        }

        None
    }

    /// Analyze timing patterns in recorded diagnostics.
    pub fn analyze_timing_patterns(&self) -> TimingAnalysis {
        if self.diagnostics.is_empty() {
            return TimingAnalysis::default();
        }

        let mut send_times = Vec::new();
        let mut receive_times = Vec::new();
        let mut failed_operations = 0;

        for diag in &self.diagnostics {
            match diag.operation {
                HidOperation::Send => send_times.push(diag.duration),
                HidOperation::Receive => receive_times.push(diag.duration),
                _ => {}
            }

            if !diag.success {
                failed_operations += 1;
            }
        }

        TimingAnalysis {
            total_operations: self.diagnostics.len(),
            failed_operations,
            avg_send_time: average_duration(&send_times),
            max_send_time: send_times.iter().max().copied().unwrap_or_default(),
            avg_receive_time: average_duration(&receive_times),
            max_receive_time: receive_times.iter().max().copied().unwrap_or_default(),
        }
    }

    /// Get diagnostic summary.
    pub fn get_summary(&self) -> String {
        let analysis = self.analyze_timing_patterns();

        format!(
            "HID Diagnostics Summary:\n\
             Total Operations: {}\n\
             Failed Operations: {} ({:.1}%)\n\
             Avg Send Time: {:.2}ms\n\
             Max Send Time: {:.2}ms\n\
             Avg Receive Time: {:.2}ms\n\
             Max Receive Time: {:.2}ms\n\
             Log File: {}",
            analysis.total_operations,
            analysis.failed_operations,
            (analysis.failed_operations as f64 / analysis.total_operations as f64) * 100.0,
            analysis.avg_send_time.as_secs_f64() * 1000.0,
            analysis.max_send_time.as_secs_f64() * 1000.0,
            analysis.avg_receive_time.as_secs_f64() * 1000.0,
            analysis.max_receive_time.as_secs_f64() * 1000.0,
            self.output_file
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "None".to_string())
        )
    }
}

/// Timing analysis results.
#[derive(Debug, Default)]
pub struct TimingAnalysis {
    pub total_operations: usize,
    pub failed_operations: usize,
    pub avg_send_time: Duration,
    pub max_send_time: Duration,
    pub avg_receive_time: Duration,
    pub max_receive_time: Duration,
}

fn average_duration(durations: &[Duration]) -> Duration {
    if durations.is_empty() {
        return Duration::default();
    }

    let total_nanos: u64 = durations.iter().map(|d| d.as_nanos() as u64).sum();
    Duration::from_nanos(total_nanos / durations.len() as u64)
}

/// Global diagnostic instance for easy access.
static GLOBAL_DIAGNOSTICS: std::sync::OnceLock<parking_lot::Mutex<HidDiagnostics>> = std::sync::OnceLock::new();

/// Initialize global diagnostics.
pub fn init_global_diagnostics(enabled: bool) -> Result<()> {
    GLOBAL_DIAGNOSTICS.get_or_init(|| parking_lot::Mutex::new(HidDiagnostics::new(enabled)));

    if enabled {
        with_global_diagnostics(|diag| diag.enable_file_logging())
            .ok_or_else(|| Error::Other("Failed to initialize diagnostics".to_string()))??;
    }

    Ok(())
}

/// Access global diagnostics instance safely.
pub fn with_global_diagnostics<F, R>(func: F) -> Option<R>
where
    F: FnOnce(&mut HidDiagnostics) -> R,
{
    GLOBAL_DIAGNOSTICS.get().map(|mutex| {
        let mut diag = mutex.lock();
        func(&mut diag)
    })
}

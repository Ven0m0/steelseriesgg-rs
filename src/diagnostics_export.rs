//! Bug report generation and diagnostic data export.
//!
//! This module provides comprehensive diagnostic data collection for bug reporting,
//! including system information, device states, HID logs, and performance metrics.
//!
//! PRIVACY WARNING: Bug reports may contain:
//! - Device serial numbers and identifiers
//! - System paths and usernames
//! - Performance data and timing information
//!   Please review the generated file before sharing publicly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sysinfo::System;
use tracing::{debug, warn};

use crate::device_state::{DeviceId, DeviceState, DeviceStateStore};
use crate::devices::{DeviceInfo, DeviceManager};
use crate::performance::RgbTimingMetrics;
use crate::{Error, Result};
use std::path::PathBuf;

/// Top-level bug report structure containing all diagnostic information.
#[derive(Debug, Serialize, Deserialize)]
pub struct BugReport {
    /// Report generation timestamp (UTC)
    pub timestamp: DateTime<Utc>,
    /// System information (OS, kernel, CPU, memory)
    pub system_info: SystemInfo,
    /// Connected device states and configurations
    pub device_states: Vec<DeviceSnapshot>,
    /// Performance metrics snapshot (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_metrics: Option<RgbTimingMetrics>,
    /// Recent error logs captured through the in-process global error log.
    pub recent_errors: Vec<ErrorLog>,
    /// HID communication logs (if included)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hid_logs: Option<Vec<String>>,
}

/// System information snapshot.
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Operating system name (e.g., "Linux", "Windows")
    pub os_name: String,
    /// Kernel version
    pub kernel_version: String,
    /// Number of CPU cores
    pub cpu_count: usize,
    /// CPU brand string
    pub cpu_brand: String,
    /// Total system memory in KB
    pub total_memory_kb: u64,
    /// Used system memory in KB
    pub used_memory_kb: u64,
    /// System uptime in seconds
    pub uptime_seconds: u64,
}

/// Device snapshot combining DeviceInfo and current state.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceSnapshot {
    /// Device identification
    pub device_id: DeviceId,
    /// Device information
    pub device_info: DeviceInfoSummary,
    /// Current device state (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_state: Option<DeviceState>,
    /// Connection status
    pub connected: bool,
}

/// Simplified device information for serialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfoSummary {
    /// Device name
    pub name: String,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Interface number
    pub interface_number: i32,
    /// Serial number (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
}

impl From<&DeviceInfo> for DeviceInfoSummary {
    fn from(info: &DeviceInfo) -> Self {
        Self {
            name: info.name.to_string(),
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            interface_number: info.interface_number,
            serial_number: info.serial_number.clone(),
        }
    }
}

/// Error log entry exported in bug reports and diagnostics snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLog {
    /// Error timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message
    pub message: String,
}

/// Global error log to keep track of recent errors.
static GLOBAL_ERROR_LOG: std::sync::OnceLock<parking_lot::Mutex<Vec<ErrorLog>>> = std::sync::OnceLock::new();

/// Maximum number of errors to keep in the global log.
const MAX_ERROR_LOGS: usize = 100;

/// Record a new error message in the global error log.
pub fn record_error(message: String) {
    let log_mutex = GLOBAL_ERROR_LOG.get_or_init(|| parking_lot::Mutex::new(Vec::new()));
    let mut logs = log_mutex.lock();

    logs.push(ErrorLog {
        timestamp: Utc::now(),
        message,
    });

    // Keep only the most recent errors
    if logs.len() > MAX_ERROR_LOGS {
        let excess = logs.len() - MAX_ERROR_LOGS;
        logs.drain(0..excess);
    }
}

/// Retrieve a copy of the recent error logs.
pub fn get_recent_errors() -> Vec<ErrorLog> {
    if let Some(log_mutex) = GLOBAL_ERROR_LOG.get() {
        log_mutex.lock().clone()
    } else {
        Vec::new()
    }
}

/// Collect system information using sysinfo (BLOCKING operation).
///
/// This function performs CPU-intensive operations and should be called
/// within tokio::task::spawn_blocking from async contexts.
fn collect_system_info_blocking() -> Result<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
    let cpu_count = sys.cpus().len();
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let total_memory_kb = sys.total_memory();
    let used_memory_kb = sys.used_memory();
    let uptime_seconds = System::uptime();

    debug!(
        "Collected system info: {} {} ({}), {} CPUs, {}KB/{} KB memory",
        os_name, kernel_version, cpu_brand, cpu_count, used_memory_kb, total_memory_kb
    );

    Ok(SystemInfo {
        os_name,
        kernel_version,
        cpu_count,
        cpu_brand,
        total_memory_kb,
        used_memory_kb,
        uptime_seconds,
    })
}

/// Collect device states from connected devices and state store (async).
async fn collect_device_states() -> Result<Vec<DeviceSnapshot>> {
    let mut device_manager = DeviceManager::new()?;
    device_manager.refresh()?;

    let devices = device_manager.devices();
    debug!("Collecting state for {} connected devices", devices.len());

    // Initialize device state store to query current states
    let state_store = DeviceStateStore::new().map_err(|e| {
        warn!("Failed to initialize device state store: {}", e);
        e
    });

    let mut snapshots = Vec::new();

    for device_info in devices {
        let device_id = DeviceId::from(device_info);
        let current_state = if let Ok(ref store) = state_store {
            store.get(&device_id)
        } else {
            None
        };

        snapshots.push(DeviceSnapshot {
            device_id,
            device_info: DeviceInfoSummary::from(device_info),
            current_state,
            connected: true,
        });
    }

    debug!("Collected {} device snapshots", snapshots.len());
    Ok(snapshots)
}

/// Collect performance metrics snapshot (async).
///
/// Returns None if no performance monitoring is active.
async fn collect_performance_snapshot() -> Result<Option<RgbTimingMetrics>> {
    // Placeholder: Would require integration with active performance monitoring session
    // For now, return None since we don't have a global performance manager
    debug!("Performance snapshot requested but no active monitoring session");
    Ok(None)
}

/// Read HID diagnostic logs if available (async).
async fn collect_hid_logs() -> Result<Option<Vec<String>>> {
    // Look for HID diagnostic log files in the secure logs directory
    // Pattern: ssgg_hid_diagnostics_*.log

    let log_dir = if let Some(config_dir) = crate::config::Config::config_dir() {
        config_dir.join("logs")
    } else {
        PathBuf::from("logs")
    };

    // Use glob to find matching files
    let mut log_files = Vec::new();

    // Use tokio::fs to read directory asynchronously
    match tokio::fs::read_dir(&log_dir).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if let Some(filename) = path.file_name() {
                    if let Some(name) = filename.to_str() {
                        if name.starts_with("ssgg_hid_diagnostics_") && name.ends_with(".log") {
                            log_files.push(path);
                        }
                    }
                }
            }
        }
        Err(e) => {
            warn!("Failed to read directory for HID logs: {}", e);
            return Ok(None);
        }
    }

    if log_files.is_empty() {
        debug!("No HID diagnostic logs found");
        return Ok(None);
    }

    // Read the most recent log file
    log_files.sort_by(|a, b| b.cmp(a)); // Sort descending by filename (timestamp)
    let latest_log = &log_files[0];

    debug!("Reading HID log from: {:?}", latest_log);

    match tokio::fs::read_to_string(latest_log).await {
        Ok(content) => {
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            debug!("Collected {} HID log lines", lines.len());
            Ok(Some(lines))
        }
        Err(e) => {
            warn!("Failed to read HID log file: {}", e);
            Ok(None)
        }
    }
}

/// Collect comprehensive bug report data (ASYNC).
///
/// This function coordinates both async and blocking operations:
/// - System info collection uses spawn_blocking (CPU-intensive sysinfo operations)
/// - Device state collection is async
/// - File I/O for HID logs is async
pub async fn collect_bug_report(include_hid_logs: bool, include_performance: bool) -> Result<BugReport> {
    debug!(
        "Collecting bug report (hid_logs={}, performance={})",
        include_hid_logs, include_performance
    );

    // Collect system info in blocking task (sysinfo is CPU-intensive)
    let system_info = tokio::task::spawn_blocking(collect_system_info_blocking)
        .await
        .map_err(|e| Error::Other(format!("Failed to spawn blocking task: {}", e)))??;

    // Collect device states (async)
    let device_states = collect_device_states().await.unwrap_or_else(|e| {
        warn!("Failed to collect device states: {}", e);
        Vec::new()
    });

    // Collect performance metrics if requested (async)
    let performance_metrics = if include_performance {
        collect_performance_snapshot().await.unwrap_or_else(|e| {
            warn!("Failed to collect performance metrics: {}", e);
            None
        })
    } else {
        None
    };

    // Collect HID logs if requested (async)
    let hid_logs = if include_hid_logs {
        collect_hid_logs().await.unwrap_or_else(|e| {
            warn!("Failed to collect HID logs: {}", e);
            None
        })
    } else {
        None
    };

    // Fetch recent error logs
    let recent_errors = get_recent_errors();

    Ok(BugReport {
        timestamp: Utc::now(),
        system_info,
        device_states,
        performance_metrics,
        recent_errors,
        hid_logs,
    })
}

/// Export bug report to JSON file (ASYNC).
pub async fn export_bug_report(report: &BugReport, path: &str) -> Result<()> {
    debug!("Exporting bug report to: {}", path);

    // Serialize to pretty-printed JSON
    let json_content = serde_json::to_string_pretty(report)
        .map_err(|e| Error::SerializationMessage(format!("Failed to serialize report: {}", e)))?;

    // Write to file using async I/O
    tokio::fs::write(path, &json_content)
        .await
        .map_err(|e| Error::FileSystemError(format!("Failed to write report: {}", e)))?;

    // Get file size for user feedback
    let file_size = json_content.len();
    let file_size_kb = file_size / 1024;

    println!("Bug report exported successfully:");
    println!("  Path: {}", path);
    println!("  Size: {} KB ({} bytes)", file_size_kb, file_size);
    println!();
    println!("PRIVACY REMINDER:");
    println!("  This report may contain device serial numbers, system paths,");
    println!("  and other potentially identifying information.");
    println!("  Please review the file before sharing publicly.");

    debug!("Bug report export complete: {} bytes", file_size);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Since GLOBAL_ERROR_LOG is static, tests running in parallel can conflict.
    // We use a local static Mutex to serialize access to the global error log in tests.
    static TEST_MUTEX: parking_lot::Mutex<()> = parking_lot::const_mutex(());

    fn clear_global_logs() {
        if let Some(log_mutex) = GLOBAL_ERROR_LOG.get() {
            log_mutex.lock().clear();
        }
    }

    #[test]
    fn test_record_and_get_errors() {
        let _guard = TEST_MUTEX.lock();
        clear_global_logs();

        record_error("Test error 1".to_string());
        record_error("Test error 2".to_string());

        let errors = get_recent_errors();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].message, "Test error 1");
        assert_eq!(errors[1].message, "Test error 2");
    }

    #[test]
    fn test_error_log_size_limit() {
        let _guard = TEST_MUTEX.lock();
        clear_global_logs();

        // Add more than the limit
        for i in 0..110 {
            record_error(format!("Error {}", i));
        }

        let errors = get_recent_errors();
        assert_eq!(errors.len(), MAX_ERROR_LOGS);

        // Ensure we kept the most recent ones
        // Since we insert sequentially, if we had 110 (0..109), the ones kept are 10..109
        assert_eq!(errors[0].message, "Error 10");
        assert_eq!(errors[errors.len() - 1].message, "Error 109");
    }
}

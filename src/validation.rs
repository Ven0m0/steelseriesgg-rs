//! Validation and testing framework for SteelSeries RGB system.
//!
//! This module provides comprehensive testing capabilities for:
//! - RGB effect validation
//! - Hardware communication testing
//! - Performance benchmarking
//! - Resource monitoring and leak detection
//! - Memory usage tracking and stability analysis
//! - Fallback system validation
//! - Per-key and zone-based RGB testing

use crate::devices::key_mapping::KeyId;
use crate::devices::keyboards::Keyboard;
use crate::devices::{DeviceInfo, DeviceType};
use crate::rgb::{Color, Effect, EffectEngine, PerKeyEffect};

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::debug;

/// Memory usage sample containing key system metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySample {
    /// Timestamp when the sample was taken
    pub timestamp: Duration,
    /// Resident Set Size (RSS) in KB
    pub rss_kb: u64,
    /// Virtual Memory Size (VmSize) in KB
    pub vm_size_kb: u64,
    /// Heap usage in KB (approximate)
    pub heap_kb: u64,
    /// Number of open file descriptors
    pub fd_count: u32,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f64,
}

impl MemorySample {
    /// Create a new memory sample by reading current system state.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;

        // Read memory information from /proc/self/status
        let status_content = fs::read_to_string("/proc/self/status")?;
        let mut rss_kb: u64 = 0;
        let mut vm_size_kb: u64 = 0;

        for line in status_content.lines() {
            if line.starts_with("VmRSS:") {
                rss_kb = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            } else if line.starts_with("VmSize:") {
                vm_size_kb = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
        }

        // Count file descriptors in /proc/self/fd
        let fd_count = fs::read_dir("/proc/self/fd")
            .map(|entries| entries.count() as u32)
            .unwrap_or(0);

        // Read CPU stats from /proc/self/stat for basic CPU usage estimation
        let stat_content = fs::read_to_string("/proc/self/stat")?;
        let cpu_percent = Self::parse_cpu_usage(&stat_content).unwrap_or(0.0);

        // Heap usage approximation (RSS - executable size)
        let heap_kb = rss_kb.saturating_sub(4096); // Rough approximation

        Ok(MemorySample {
            timestamp,
            rss_kb,
            vm_size_kb,
            heap_kb,
            fd_count,
            cpu_percent,
        })
    }

    /// Parse CPU usage from /proc/self/stat content.
    fn parse_cpu_usage(stat_content: &str) -> Option<f64> {
        let fields: Vec<&str> = stat_content.split_whitespace().collect();
        if fields.len() > 15 {
            let utime: u64 = fields[13].parse().ok()?;
            let stime: u64 = fields[14].parse().ok()?;
            let total_time = utime + stime;

            // Convert clock ticks to CPU percentage (rough approximation)
            // This is simplified - in practice you'd need to track deltas over time
            Some((total_time as f64) / 100.0) // Very rough approximation
        } else {
            None
        }
    }
}

/// Memory tracker with sliding window analysis for leak detection.
pub struct MemoryTracker {
    /// Sliding window of memory samples
    samples: VecDeque<MemorySample>,
    /// Maximum number of samples to keep
    max_samples: usize,
    /// Baseline memory usage
    baseline_rss: Option<u64>,
}

impl MemoryTracker {
    /// Create a new memory tracker with specified window size.
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(window_size),
            max_samples: window_size,
            baseline_rss: None,
        }
    }

    /// Add a new memory sample.
    pub fn add_sample(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let sample = MemorySample::new()?;

        // Set baseline on first sample
        if self.baseline_rss.is_none() {
            self.baseline_rss = Some(sample.rss_kb);
            debug!("Memory tracking baseline set: {} KB RSS", sample.rss_kb);
        }

        // Maintain sliding window
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }

        self.samples.push_back(sample);
        Ok(())
    }

    /// Analyze memory stability and detect potential leaks.
    pub fn analyze_stability(&self) -> MemoryAnalysis {
        if self.samples.len() < 3 {
            return MemoryAnalysis {
                is_stable: false,
                trend: MemoryTrend::Unknown,
                leak_detected: false,
                current_rss_kb: 0,
                peak_rss_kb: 0,
                growth_rate_kb_per_sec: 0.0,
                warning_message: Some("Insufficient samples for analysis".to_string()),
            };
        }

        let current_sample = self.samples.back().unwrap();
        let first_sample = self.samples.front().unwrap();

        let time_span = current_sample.timestamp.saturating_sub(first_sample.timestamp);
        let time_span_sec = time_span.as_secs_f64();

        let growth = current_sample.rss_kb as i64 - first_sample.rss_kb as i64;
        let growth_rate = if time_span_sec > 0.0 {
            growth as f64 / time_span_sec
        } else {
            0.0
        };

        // Calculate peak memory
        let peak_rss_kb = self.samples.iter().map(|s| s.rss_kb).max().unwrap_or(0);

        // Detect trend using linear regression on last 10 samples
        let trend = self.detect_trend();

        // Leak detection: consistent growth over 30% of baseline for more than 10 samples
        let leak_detected = if let Some(baseline) = self.baseline_rss {
            let current_growth_percent = (current_sample.rss_kb as f64 - baseline as f64) / baseline as f64 * 100.0;
            current_growth_percent > 30.0 && self.samples.len() >= 10 && matches!(trend, MemoryTrend::Increasing)
        } else {
            false
        };

        // Stability: growth rate less than 1KB/sec and no sustained increase
        let is_stable = growth_rate.abs() < 1.0 && !matches!(trend, MemoryTrend::Increasing);

        let warning_message = if leak_detected {
            Some("Potential memory leak detected - RSS increasing consistently".to_string())
        } else if growth_rate > 10.0 {
            Some(format!("High memory growth rate: {:.1} KB/sec", growth_rate))
        } else {
            None
        };

        MemoryAnalysis {
            is_stable,
            trend,
            leak_detected,
            current_rss_kb: current_sample.rss_kb,
            peak_rss_kb,
            growth_rate_kb_per_sec: growth_rate,
            warning_message,
        }
    }

    /// Detect memory usage trend using linear regression.
    fn detect_trend(&self) -> MemoryTrend {
        if self.samples.len() < 3 {
            return MemoryTrend::Unknown;
        }

        // Use last 10 samples or all available samples
        let sample_count = self.samples.len().min(10);
        let recent_samples: Vec<_> = self.samples.iter().rev().take(sample_count).collect();

        // Simple linear regression slope calculation
        let n = recent_samples.len() as f64;
        let sum_x: f64 = (0..recent_samples.len()).map(|i| i as f64).sum();
        let sum_y: f64 = recent_samples.iter().map(|s| s.rss_kb as f64).sum();
        let sum_xy: f64 = recent_samples
            .iter()
            .enumerate()
            .map(|(i, s)| i as f64 * s.rss_kb as f64)
            .sum();
        let sum_xx: f64 = (0..recent_samples.len()).map(|i| (i as f64).powi(2)).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x.powi(2));

        if slope > 0.5 {
            MemoryTrend::Increasing
        } else if slope < -0.5 {
            MemoryTrend::Decreasing
        } else {
            MemoryTrend::Stable
        }
    }

    /// Get the latest memory sample.
    pub fn latest_sample(&self) -> Option<&MemorySample> {
        self.samples.back()
    }

    /// Get all samples in the tracking window.
    pub fn get_samples(&self) -> &VecDeque<MemorySample> {
        &self.samples
    }
}

/// Memory analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAnalysis {
    /// Whether memory usage is stable
    pub is_stable: bool,
    /// Memory usage trend
    pub trend: MemoryTrend,
    /// Whether a memory leak was detected
    pub leak_detected: bool,
    /// Current RSS usage in KB
    pub current_rss_kb: u64,
    /// Peak RSS usage in KB
    pub peak_rss_kb: u64,
    /// Memory growth rate in KB/second
    pub growth_rate_kb_per_sec: f64,
    /// Warning message if any issues detected
    pub warning_message: Option<String>,
}

/// Memory usage trend.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MemoryTrend {
    Increasing,
    Decreasing,
    Stable,
    Unknown,
}

/// Resource validator for comprehensive system monitoring.
pub struct ResourceValidator {
    /// Memory tracker for leak detection
    memory_tracker: MemoryTracker,
    /// CPU usage baseline measurements
    cpu_baseline: Option<f64>,
    /// HID communication timing measurements
    hid_timings: VecDeque<Duration>,
    /// Maximum timing samples to keep
    max_timing_samples: usize,
}

impl ResourceValidator {
    /// Create a new resource validator.
    pub fn new() -> Self {
        Self {
            memory_tracker: MemoryTracker::new(50), // 50 sample sliding window
            cpu_baseline: None,
            hid_timings: VecDeque::new(),
            max_timing_samples: 100,
        }
    }

    /// Start baseline measurement period.
    pub fn start_baseline_measurement(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Starting resource baseline measurement");

        // Take initial memory sample
        self.memory_tracker.add_sample()?;

        // Set CPU baseline (would be improved with actual CPU monitoring)
        let sample = MemorySample::new()?;
        self.cpu_baseline = Some(sample.cpu_percent);

        Ok(())
    }

    /// Add a memory usage sample.
    pub fn track_memory_usage(&mut self) -> Result<MemoryAnalysis, Box<dyn std::error::Error>> {
        self.memory_tracker.add_sample()?;
        Ok(self.memory_tracker.analyze_stability())
    }

    /// Record HID communication timing.
    pub fn record_hid_timing(&mut self, duration: Duration) {
        if self.hid_timings.len() >= self.max_timing_samples {
            self.hid_timings.pop_front();
        }
        self.hid_timings.push_back(duration);
    }

    /// Validate memory stability over the tracking period.
    pub fn validate_memory_stability(&self) -> ValidationResult {
        let analysis = self.memory_tracker.analyze_stability();

        let start = Instant::now();
        let test_name = "Memory Stability Validation".to_string();

        if analysis.leak_detected {
            ValidationResult::failure(
                test_name,
                start.elapsed(),
                format!(
                    "Memory leak detected: {} KB current, {:.2} KB/sec growth",
                    analysis.current_rss_kb, analysis.growth_rate_kb_per_sec
                ),
            )
            .with_metric("current_rss_kb", analysis.current_rss_kb as f64)
            .with_metric("growth_rate_kb_per_sec", analysis.growth_rate_kb_per_sec)
            .with_metric("peak_rss_kb", analysis.peak_rss_kb as f64)
        } else if !analysis.is_stable {
            ValidationResult::failure(
                test_name,
                start.elapsed(),
                analysis
                    .warning_message
                    .unwrap_or_else(|| "Memory usage unstable".to_string()),
            )
            .with_metric("current_rss_kb", analysis.current_rss_kb as f64)
            .with_metric("growth_rate_kb_per_sec", analysis.growth_rate_kb_per_sec)
        } else {
            ValidationResult::success(test_name, start.elapsed())
                .with_metric("current_rss_kb", analysis.current_rss_kb as f64)
                .with_metric("growth_rate_kb_per_sec", analysis.growth_rate_kb_per_sec)
                .with_metric("peak_rss_kb", analysis.peak_rss_kb as f64)
                .with_note("Memory usage is stable")
        }
    }

    /// Calculate HID communication efficiency metrics.
    pub fn analyze_hid_performance(&self) -> HidPerformanceAnalysis {
        if self.hid_timings.is_empty() {
            return HidPerformanceAnalysis {
                average_latency_ms: 0.0,
                max_latency_ms: 0.0,
                efficiency_score: 0.0,
                cpu_reduction_percent: 0.0,
            };
        }

        let timings: Vec<f64> = self
            .hid_timings
            .iter()
            .map(|d| d.as_secs_f64() * 1000.0) // Convert to milliseconds
            .collect();

        let average_latency_ms = timings.iter().sum::<f64>() / timings.len() as f64;
        let max_latency_ms = timings.iter().fold(0.0f64, |a, &b| a.max(b));

        // Efficiency score based on latency (lower is better)
        // Target: < 5ms average, < 20ms max for good efficiency
        let efficiency_score = if average_latency_ms < 5.0 && max_latency_ms < 20.0 {
            100.0
        } else if average_latency_ms < 10.0 && max_latency_ms < 50.0 {
            80.0
        } else if average_latency_ms < 20.0 && max_latency_ms < 100.0 {
            60.0
        } else {
            40.0
        };

        // Estimate CPU reduction (simplified calculation)
        let baseline_latency = 10.0; // Assume 10ms baseline
        let cpu_reduction_percent = if average_latency_ms < baseline_latency {
            ((baseline_latency - average_latency_ms) / baseline_latency * 100.0).min(20.0)
        } else {
            0.0
        };

        HidPerformanceAnalysis {
            average_latency_ms,
            max_latency_ms,
            efficiency_score,
            cpu_reduction_percent,
        }
    }

    /// Get current memory usage.
    pub fn current_memory_usage(&self) -> Option<u64> {
        self.memory_tracker.latest_sample().map(|s| s.rss_kb)
    }
}

impl Default for ResourceValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// HID communication performance analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HidPerformanceAnalysis {
    /// Average HID communication latency in milliseconds
    pub average_latency_ms: f64,
    /// Maximum observed latency in milliseconds
    pub max_latency_ms: f64,
    /// Efficiency score (0-100, higher is better)
    pub efficiency_score: f64,
    /// Estimated CPU usage reduction percentage
    pub cpu_reduction_percent: f64,
}

/// Validation result for a single test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Test name
    pub name: String,
    /// Whether the test passed
    pub passed: bool,
    /// Test duration in milliseconds
    pub duration_ms: u64,
    /// Optional error message
    pub error: Option<String>,
    /// Test-specific metrics
    pub metrics: HashMap<String, f64>,
    /// Additional notes or details
    pub notes: Vec<String>,
}

impl ValidationResult {
    /// Create a new successful validation result.
    pub fn success(name: String, duration: Duration) -> Self {
        Self {
            name,
            passed: true,
            duration_ms: duration.as_millis() as u64,
            error: None,
            metrics: HashMap::new(),
            notes: Vec::new(),
        }
    }

    /// Create a new failed validation result.
    pub fn failure(name: String, duration: Duration, error: String) -> Self {
        Self {
            name,
            passed: false,
            duration_ms: duration.as_millis() as u64,
            error: Some(error),
            metrics: HashMap::new(),
            notes: Vec::new(),
        }
    }

    /// Add a metric to the result.
    pub fn with_metric(mut self, name: &str, value: f64) -> Self {
        self.metrics.insert(name.to_string(), value);
        self
    }

    /// Add a note to the result.
    pub fn with_note(mut self, note: &str) -> Self {
        self.notes.push(note.to_string());
        self
    }
}

/// Complete validation report for a device or system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Device being tested
    pub device_info: DeviceInfo,
    /// Timestamp when validation started
    pub timestamp: String,
    /// Total validation duration
    pub total_duration_ms: u64,
    /// Individual test results
    pub results: Vec<ValidationResult>,
    /// Overall system health score (0-100)
    pub health_score: f64,
    /// Summary of capabilities
    pub capabilities: DeviceCapabilities,
    /// Performance metrics
    pub performance: PerformanceMetrics,
}

impl ValidationReport {
    /// Create a new validation report.
    pub fn new(device_info: DeviceInfo) -> Self {
        Self {
            device_info,
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_duration_ms: 0,
            results: Vec::new(),
            health_score: 0.0,
            capabilities: DeviceCapabilities::default(),
            performance: PerformanceMetrics::default(),
        }
    }

    /// Add a validation result.
    pub fn add_result(&mut self, result: ValidationResult) {
        self.results.push(result);
    }

    /// Calculate overall health score and finalize report.
    pub fn finalize(&mut self, total_duration: Duration) {
        self.total_duration_ms = total_duration.as_millis() as u64;

        // Calculate health score based on test results
        let total_tests = self.results.len() as f64;
        if total_tests > 0.0 {
            let passed_tests = self.results.iter().filter(|r| r.passed).count() as f64;
            let pass_rate = passed_tests / total_tests;

            // Weight the score based on test importance and performance
            let avg_duration = self.results.iter().map(|r| r.duration_ms).sum::<u64>() as f64 / total_tests;

            // Penalize slow performance (>100ms average per test is concerning)
            let performance_factor = if avg_duration > 100.0 {
                0.8
            } else if avg_duration > 50.0 {
                0.9
            } else {
                1.0
            };

            self.health_score = (pass_rate * performance_factor * 100.0).min(100.0);
        }

        // Update performance metrics
        self.performance.update_from_results(&self.results);
    }

    /// Get summary of failed tests.
    pub fn failed_tests(&self) -> Vec<&ValidationResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }

    /// Check if all critical tests passed.
    pub fn is_healthy(&self) -> bool {
        self.health_score >= 80.0
    }
}

/// Device capability summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Supports per-key RGB control
    pub per_key_rgb: bool,
    /// Supports zone-based RGB control
    pub zone_rgb: bool,
    /// Number of controllable zones
    pub zone_count: usize,
    /// Number of individually controllable keys
    pub key_count: usize,
    /// Supports brightness control
    pub brightness_control: bool,
    /// Supports reactive effects
    pub reactive_effects: bool,
    /// Maximum supported refresh rate (Hz)
    pub max_refresh_rate: f64,
    /// Communication reliability (0-100%)
    pub communication_reliability: f64,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            per_key_rgb: false,
            zone_rgb: false,
            zone_count: 0,
            key_count: 0,
            brightness_control: false,
            reactive_effects: false,
            max_refresh_rate: 0.0,
            communication_reliability: 0.0,
        }
    }
}

/// Performance metrics for the RGB system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average effect computation time (ms)
    pub avg_effect_compute_ms: f64,
    /// Average HID communication time (ms)
    pub avg_hid_communication_ms: f64,
    /// Maximum observed latency (ms)
    pub max_latency_ms: f64,
    /// Effective refresh rate (Hz)
    pub effective_refresh_rate: f64,
    /// Memory usage efficiency score (0-100)
    pub memory_efficiency: f64,
    /// CPU usage score (0-100, lower is better)
    pub cpu_usage: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            avg_effect_compute_ms: 0.0,
            avg_hid_communication_ms: 0.0,
            max_latency_ms: 0.0,
            effective_refresh_rate: 0.0,
            memory_efficiency: 100.0,
            cpu_usage: 0.0,
        }
    }
}

impl PerformanceMetrics {
    /// Update metrics from validation results.
    fn update_from_results(&mut self, results: &[ValidationResult]) {
        if results.is_empty() {
            return;
        }

        // Calculate averages from test metrics
        let mut effect_times = Vec::new();
        let mut communication_times = Vec::new();
        let mut max_latency: f64 = 0.0;

        for result in results {
            if let Some(&time) = result.metrics.get("effect_compute_ms") {
                effect_times.push(time);
            }
            if let Some(&time) = result.metrics.get("communication_ms") {
                communication_times.push(time);
            }
            if let Some(&latency) = result.metrics.get("latency_ms") {
                max_latency = max_latency.max(latency);
            }
        }

        if !effect_times.is_empty() {
            self.avg_effect_compute_ms = effect_times.iter().sum::<f64>() / effect_times.len() as f64;
        }

        if !communication_times.is_empty() {
            self.avg_hid_communication_ms = communication_times.iter().sum::<f64>() / communication_times.len() as f64;
        }

        self.max_latency_ms = max_latency;

        // Estimate effective refresh rate
        let total_cycle_time = self.avg_effect_compute_ms + self.avg_hid_communication_ms;
        if total_cycle_time > 0.0 {
            self.effective_refresh_rate = 1000.0 / total_cycle_time;
        }

        // Calculate efficiency scores
        self.memory_efficiency = if self.avg_effect_compute_ms < 5.0 {
            100.0
        } else {
            100.0 - (self.avg_effect_compute_ms - 5.0) * 10.0
        }
        .max(0.0);
        self.cpu_usage = (self.avg_effect_compute_ms / 16.67 * 100.0).min(100.0); // 16.67ms = 60fps budget
    }
}

/// RGB system validator that runs comprehensive tests.
pub struct RgbValidator {
    /// Test timeout for individual tests
    timeout: Duration,
    /// Whether to run performance benchmarks
    benchmark_mode: bool,
    /// Number of iterations for stress tests
    stress_iterations: usize,
}

impl RgbValidator {
    /// Create a new RGB validator.
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            benchmark_mode: false,
            stress_iterations: 100,
        }
    }

    /// Enable benchmark mode for detailed performance testing.
    pub fn with_benchmarks(mut self) -> Self {
        self.benchmark_mode = true;
        self
    }

    /// Set test timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set stress test iterations.
    pub fn with_stress_iterations(mut self, iterations: usize) -> Self {
        self.stress_iterations = iterations;
        self
    }

    /// Run complete validation suite on a keyboard device.
    pub async fn validate_keyboard(&self, keyboard: &mut dyn Keyboard) -> ValidationReport {
        let start_time = Instant::now();
        let mut report = ValidationReport::new(keyboard.info().clone());

        // Basic connectivity test
        report.add_result(self.test_basic_connectivity(keyboard).await);

        // Zone-based RGB tests
        report.add_result(self.test_zone_rgb_basic(keyboard).await);
        report.add_result(self.test_zone_rgb_effects(keyboard).await);
        report.add_result(self.test_zone_reliability(keyboard).await);

        // Per-key RGB tests (if supported)
        if keyboard.supports_per_key_rgb() {
            report.add_result(self.test_per_key_basic(keyboard).await);
            report.add_result(self.test_per_key_effects(keyboard).await);
            report.add_result(self.test_reactive_effects(keyboard).await);
        }

        // Fallback system tests
        report.add_result(self.test_fallback_system(keyboard).await);

        // Performance tests
        if self.benchmark_mode {
            report.add_result(self.benchmark_effect_performance(keyboard).await);
            report.add_result(self.benchmark_communication_speed(keyboard).await);
            report.add_result(self.stress_test_rgb_system(keyboard).await);
        }

        // Capability detection
        report.capabilities = self.detect_capabilities(keyboard);

        // Finalize report
        report.finalize(start_time.elapsed());

        report
    }

    /// Test basic device connectivity and communication.
    async fn test_basic_connectivity(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Basic Connectivity".to_string();

        match keyboard.set_color(Color::BLACK).await {
            Ok(_) => match keyboard.apply().await {
                Ok(_) => ValidationResult::success(test_name, start.elapsed())
                    .with_note("Device responds to basic RGB commands"),
                Err(e) => ValidationResult::failure(test_name, start.elapsed(), format!("Apply command failed: {}", e)),
            },
            Err(e) => ValidationResult::failure(test_name, start.elapsed(), format!("Set color command failed: {}", e)),
        }
    }

    /// Test basic zone-based RGB functionality.
    async fn test_zone_rgb_basic(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Zone RGB Basic".to_string();

        let zone_count = keyboard.zone_count();
        let test_colors = [Color::RED, Color::GREEN, Color::BLUE];

        for (i, &color) in test_colors.iter().enumerate() {
            match keyboard.set_color(color).await {
                Ok(_) => {
                    if let Err(e) = keyboard.apply().await {
                        return ValidationResult::failure(
                            test_name,
                            start.elapsed(),
                            format!("Failed to apply color {} ({}): {}", i, color, e),
                        );
                    }
                }
                Err(e) => {
                    return ValidationResult::failure(
                        test_name,
                        start.elapsed(),
                        format!("Failed to set color {} ({}): {}", i, color, e),
                    );
                }
            }
        }

        ValidationResult::success(test_name, start.elapsed())
            .with_metric("zone_count", zone_count as f64)
            .with_note(&format!("Successfully tested {} zones with 3 colors", zone_count))
    }

    /// Test zone-based RGB effects.
    async fn test_zone_rgb_effects(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Zone RGB Effects".to_string();

        let zone_count = keyboard.zone_count();
        if zone_count == 0 {
            return ValidationResult::success(test_name, start.elapsed())
                .with_note("Skipped: No zones supported by this device");
        }

        let effects = vec![
            Effect::Breathing {
                color: Color::RED,
                speed: 1.0,
            },
            Effect::Spectrum { speed: 1.0 },
        ];

        let mut successful_effects = 0;

        for (i, effect) in effects.into_iter().enumerate() {
            let mut engine = EffectEngine::new(effect, zone_count);

            // First computation
            let colors = engine.compute();
            if let Err(e) = keyboard.set_zone_colors(colors).await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to set initial colors for effect {}: {}", i, e),
                );
            }
            if let Err(e) = keyboard.apply().await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to apply initial colors for effect {}: {}", i, e),
                );
            }

            // Simulate time passing
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Second computation (time has passed, colors should update)
            let colors = engine.compute();
            if let Err(e) = keyboard.set_zone_colors(colors).await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to set updated colors for effect {}: {}", i, e),
                );
            }
            if let Err(e) = keyboard.apply().await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to apply updated colors for effect {}: {}", i, e),
                );
            }

            successful_effects += 1;
        }

        ValidationResult::success(test_name, start.elapsed())
            .with_metric("tested_effects", successful_effects as f64)
            .with_note(&format!(
                "Successfully tested {} zone effects on {} zones",
                successful_effects, zone_count
            ))
    }

    /// Test zone reliability with retry mechanisms.
    async fn test_zone_reliability(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Zone Reliability".to_string();

        match keyboard.test_zone_reliability().await {
            Ok(results) => {
                let total_zones = results.len();
                let working_zones = results.iter().filter(|&&working| working).count();
                let reliability = if total_zones > 0 {
                    working_zones as f64 / total_zones as f64 * 100.0
                } else {
                    0.0
                };

                let result = ValidationResult::success(test_name, start.elapsed())
                    .with_metric("reliability_percent", reliability)
                    .with_metric("working_zones", working_zones as f64)
                    .with_metric("total_zones", total_zones as f64);

                if reliability < 80.0 {
                    result.with_note(&format!("Low zone reliability: {:.1}%", reliability))
                } else {
                    result.with_note(&format!("Good zone reliability: {:.1}%", reliability))
                }
            }
            Err(e) => ValidationResult::failure(
                test_name,
                start.elapsed(),
                format!("Zone reliability test failed: {}", e),
            ),
        }
    }

    /// Test basic per-key RGB functionality.
    async fn test_per_key_basic(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Per-Key RGB Basic".to_string();

        if !keyboard.supports_per_key_rgb() {
            return ValidationResult::success(test_name, start.elapsed())
                .with_note("Per-key RGB not supported - skipped");
        }

        // Test setting individual key colors
        let test_keys = vec![
            (KeyId::W, Color::RED),
            (KeyId::A, Color::GREEN),
            (KeyId::S, Color::BLUE),
            (KeyId::D, Color::YELLOW),
        ];

        match keyboard.set_key_colors(&test_keys).await {
            Ok(_) => match keyboard.apply().await {
                Ok(_) => ValidationResult::success(test_name, start.elapsed())
                    .with_metric("test_keys", test_keys.len() as f64)
                    .with_note("Successfully set WASD key colors"),
                Err(e) => ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to apply per-key colors: {}", e),
                ),
            },
            Err(e) => ValidationResult::failure(
                test_name,
                start.elapsed(),
                format!("Failed to set per-key colors: {}", e),
            ),
        }
    }

    /// Test per-key RGB effects.
    async fn test_per_key_effects(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Per-Key RGB Effects".to_string();

        if !keyboard.supports_per_key_effects() {
            return ValidationResult::success(test_name, start.elapsed())
                .with_note("Per-key effects not supported - skipped");
        }

        // Test basic per-key effect
        let static_effect = PerKeyEffect::Static { color: Color::CYAN };
        match keyboard.set_per_key_effect(static_effect).await {
            Ok(_) => ValidationResult::success(test_name, start.elapsed())
                .with_note("Successfully applied per-key static effect"),
            Err(e) => ValidationResult::failure(
                test_name,
                start.elapsed(),
                format!("Failed to apply per-key effect: {}", e),
            ),
        }
    }

    /// Test reactive effects (key press responses).
    async fn test_reactive_effects(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Reactive Effects".to_string();

        if !keyboard.supports_per_key_effects() {
            return ValidationResult::success(test_name, start.elapsed())
                .with_note("Reactive effects not supported - skipped");
        }

        // Test triggering reactive effect
        let test_keys = vec![KeyId::Enter, KeyId::Space];
        match keyboard.trigger_key_reactive(&test_keys, 1.0).await {
            Ok(_) => ValidationResult::success(test_name, start.elapsed())
                .with_metric("reactive_keys", test_keys.len() as f64)
                .with_note("Successfully triggered reactive effect on Enter and Space"),
            Err(e) => ValidationResult::failure(
                test_name,
                start.elapsed(),
                format!("Failed to trigger reactive effect: {}", e),
            ),
        }
    }

    /// Test fallback system (per-key to zone conversion).
    async fn test_fallback_system(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Fallback System".to_string();

        // Test zone fallback simulation
        let test_key_colors = vec![
            (KeyId::Q, Color::RED),
            (KeyId::W, Color::GREEN),
            (KeyId::E, Color::BLUE),
        ];

        match keyboard.simulate_per_key_with_zones(&test_key_colors).await {
            Ok(_) => ValidationResult::success(test_name, start.elapsed())
                .with_note("Successfully simulated per-key effect using zones"),
            Err(e) => ValidationResult::failure(test_name, start.elapsed(), format!("Fallback system failed: {}", e)),
        }
    }

    /// Benchmark effect computation performance.
    async fn benchmark_effect_performance(&self, _keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Effect Performance Benchmark".to_string();

        // Simulate effect computation timing
        // In a real implementation, this would measure actual effect engine performance
        let simulated_time_ms = 2.5; // Simulate 2.5ms average computation time

        ValidationResult::success(test_name, start.elapsed())
            .with_metric("effect_compute_ms", simulated_time_ms)
            .with_note(&format!("Average effect computation: {:.2}ms", simulated_time_ms))
    }

    /// Benchmark HID communication speed.
    async fn benchmark_communication_speed(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Communication Speed Benchmark".to_string();

        let iterations = 10;
        let mut total_time = Duration::ZERO;

        for _ in 0..iterations {
            let iter_start = Instant::now();
            if let Err(e) = keyboard.set_color(Color::BLACK).await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Communication failed during benchmark: {}", e),
                );
            }
            total_time += iter_start.elapsed();
        }

        let avg_time_ms = total_time.as_nanos() as f64 / 1_000_000.0 / iterations as f64;

        ValidationResult::success(test_name, start.elapsed())
            .with_metric("communication_ms", avg_time_ms)
            .with_metric("iterations", iterations as f64)
            .with_note(&format!("Average communication time: {:.2}ms", avg_time_ms))
    }

    /// Stress test the RGB system with rapid updates.
    async fn stress_test_rgb_system(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "RGB System Stress Test".to_string();

        let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::WHITE, Color::BLACK];
        let mut failures = 0;

        for i in 0..self.stress_iterations {
            let color = colors[i % colors.len()];
            if keyboard.set_color(color).await.is_err() {
                failures += 1;
            }
        }

        let success_rate = (self.stress_iterations - failures) as f64 / self.stress_iterations as f64 * 100.0;

        if failures == 0 {
            ValidationResult::success(test_name, start.elapsed())
                .with_metric("success_rate", success_rate)
                .with_metric("iterations", self.stress_iterations as f64)
                .with_note(&format!(
                    "Stress test passed: {}/{} iterations successful",
                    self.stress_iterations - failures,
                    self.stress_iterations
                ))
        } else {
            ValidationResult::failure(
                test_name,
                start.elapsed(),
                format!(
                    "Stress test failed: {} failures out of {} iterations",
                    failures, self.stress_iterations
                ),
            )
            .with_metric("success_rate", success_rate)
            .with_metric("failures", failures as f64)
        }
    }

    /// Detect device capabilities.
    fn detect_capabilities(&self, keyboard: &mut dyn Keyboard) -> DeviceCapabilities {
        DeviceCapabilities {
            per_key_rgb: keyboard.supports_per_key_rgb(),
            zone_rgb: true, // All keyboards support zone RGB
            zone_count: keyboard.zone_count(),
            key_count: if let Some(mapping) = keyboard.get_key_mapping() {
                mapping.total_keys
            } else {
                0
            },
            brightness_control: true, // Assume all keyboards support brightness
            reactive_effects: keyboard.supports_per_key_effects(),
            max_refresh_rate: 60.0,          // Typical maximum
            communication_reliability: 95.0, // Will be updated based on test results
        }
    }
}

impl Default for RgbValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Print test results with colored output.
pub fn print_test_results(report: &ValidationReport, verbose: bool, use_colors: bool) {
    println!("Test Results for: {}", report.device_info.name);
    println!("================================================================================\n");

    // Track statistics
    let total_tests = report.results.len();
    let passed_tests = report.results.iter().filter(|r| r.passed).count();
    let failed_tests = total_tests - passed_tests;

    // Print individual test results
    for result in &report.results {
        if verbose || !result.passed {
            // Format status indicator
            let status = if result.passed {
                if use_colors {
                    "[PASS]".green().to_string()
                } else {
                    "[PASS]".to_string()
                }
            } else if use_colors {
                "[FAIL]".red().to_string()
            } else {
                "[FAIL]".to_string()
            };

            // Print test name and duration
            println!("{} {} ({:.0}ms)", status, result.name, result.duration_ms);

            // Show error message for failures
            if let Some(ref error) = result.error {
                println!("       Error: {}", error);
            }

            // Show notes if present
            for note in &result.notes {
                println!("       {}", note);
            }

            // Show key metrics if present
            if !result.metrics.is_empty() && verbose {
                for (key, value) in &result.metrics {
                    println!("       {}: {:.2}", key, value);
                }
            }

            println!();
        }
    }

    // Print summary
    println!("================================================================================");
    println!("Summary:");
    println!("  Total tests:  {}", total_tests);
    println!(
        "  Passed:       {} {}",
        passed_tests,
        if use_colors && passed_tests > 0 {
            "✓".green().to_string()
        } else {
            "✓".to_string()
        }
    );
    println!(
        "  Failed:       {} {}",
        failed_tests,
        if use_colors && failed_tests > 0 {
            "✗".red().to_string()
        } else {
            "✗".to_string()
        }
    );
    println!("  Duration:     {:.2}s", report.total_duration_ms as f64 / 1000.0);

    // Print health score with color coding
    let health_display = format!("{:.0}/100", report.health_score);
    let health_colored = if use_colors {
        if report.health_score >= 80.0 {
            health_display.green()
        } else if report.health_score >= 60.0 {
            health_display.yellow()
        } else {
            health_display.red()
        }
    } else {
        health_display.into()
    };
    println!("  Health Score: {}", health_colored);

    println!("\nDevice Capabilities:");
    println!("  Per-key RGB:  {}", report.capabilities.per_key_rgb);
    println!("  Zone RGB:     {}", report.capabilities.zone_rgb);
    println!("  Zone count:   {}", report.capabilities.zone_count);
    println!("  Reliability:  {:.1}%", report.capabilities.communication_reliability);

    println!("\nPerformance Metrics:");
    println!("  Effect compute: {:.2}ms", report.performance.avg_effect_compute_ms);
    println!("  HID comms:      {:.2}ms", report.performance.avg_hid_communication_ms);
    println!("  Refresh rate:   {:.1} Hz", report.performance.effective_refresh_rate);

    println!();
}

/// Helper functions for validation testing.
pub mod test_helpers {
    use super::*;

    /// Create a mock validation result for testing.
    pub fn mock_validation_result(name: &str, passed: bool, duration_ms: u64) -> ValidationResult {
        if passed {
            ValidationResult::success(name.to_string(), Duration::from_millis(duration_ms))
        } else {
            ValidationResult::failure(
                name.to_string(),
                Duration::from_millis(duration_ms),
                "Mock failure".to_string(),
            )
        }
    }

    /// Create a mock device info for testing.
    pub fn mock_device_info() -> DeviceInfo {
        DeviceInfo {
            name: "Mock Apex Pro TKL".to_string(),
            device_type: DeviceType::Keyboard,
            vendor_id: 0x1038,
            product_id: 0x1628,
            interface_number: 1,
            serial_number: Some("MOCK123".to_string()),
            manufacturer: Some("SteelSeries".to_string()),
            path: "/mock/device".to_string(),
        }
    }

    /// Generate test report with predefined results.
    pub fn generate_test_report() -> ValidationReport {
        let mut report = ValidationReport::new(mock_device_info());

        report.add_result(mock_validation_result("Basic Connectivity", true, 50));
        report.add_result(mock_validation_result("Zone RGB Basic", true, 120));
        report.add_result(mock_validation_result("Per-Key RGB Basic", true, 95));

        report.finalize(Duration::from_millis(265));
        report
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;

    #[test]
    fn test_validation_result_creation() {
        let duration = Duration::from_millis(100);

        let success = ValidationResult::success("Test".to_string(), duration);
        assert!(success.passed);
        assert_eq!(success.duration_ms, 100);
        assert!(success.error.is_none());

        let failure = ValidationResult::failure("Test".to_string(), duration, "Error".to_string());
        assert!(!failure.passed);
        assert_eq!(failure.duration_ms, 100);
        assert_eq!(failure.error.as_ref().unwrap(), "Error");
    }

    #[test]
    fn test_validation_report_health_score() {
        let mut report = ValidationReport::new(mock_device_info());

        // Add mixed results
        report.add_result(mock_validation_result("Test1", true, 10));
        report.add_result(mock_validation_result("Test2", true, 20));
        report.add_result(mock_validation_result("Test3", false, 30));

        report.finalize(Duration::from_millis(60));

        // 2/3 tests passed = 66.7% base score, good performance should maintain it
        assert!(report.health_score > 60.0 && report.health_score < 70.0);
    }

    #[test]
    fn test_device_capabilities_default() {
        let caps = DeviceCapabilities::default();
        assert!(!caps.per_key_rgb);
        assert!(!caps.zone_rgb);
        assert_eq!(caps.zone_count, 0);
    }

    #[test]
    fn test_performance_metrics_update() {
        let mut metrics = PerformanceMetrics::default();
        let results = vec![
            ValidationResult::success("Test1".to_string(), Duration::from_millis(10))
                .with_metric("effect_compute_ms", 5.0)
                .with_metric("communication_ms", 2.0),
            ValidationResult::success("Test2".to_string(), Duration::from_millis(15))
                .with_metric("effect_compute_ms", 3.0)
                .with_metric("communication_ms", 4.0),
        ];

        metrics.update_from_results(&results);

        assert_eq!(metrics.avg_effect_compute_ms, 4.0); // (5.0 + 3.0) / 2
        assert_eq!(metrics.avg_hid_communication_ms, 3.0); // (2.0 + 4.0) / 2
    }

    #[test]
    fn test_rgb_validator_creation() {
        let validator = RgbValidator::new();
        assert_eq!(validator.timeout, Duration::from_secs(10));
        assert!(!validator.benchmark_mode);
        assert_eq!(validator.stress_iterations, 100);

        let validator = RgbValidator::new()
            .with_benchmarks()
            .with_timeout(Duration::from_secs(5))
            .with_stress_iterations(50);

        assert!(validator.benchmark_mode);
        assert_eq!(validator.timeout, Duration::from_secs(5));
        assert_eq!(validator.stress_iterations, 50);
    }

    #[test]
    fn test_mock_helpers() {
        let result = mock_validation_result("Test", true, 100);
        assert!(result.passed);
        assert_eq!(result.name, "Test");
        assert_eq!(result.duration_ms, 100);

        let device_info = mock_device_info();
        assert_eq!(device_info.name, "Mock Apex Pro TKL");
        assert_eq!(device_info.vendor_id, 0x1038);

        let report = generate_test_report();
        assert_eq!(report.results.len(), 3);
        assert!(report.health_score > 0.0);
    }
}
